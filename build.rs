use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use inflections::case::to_snake_case;

fn main() -> std::io::Result<()> {
    let mut generated_tests = String::new();
    let zones = [
        "Zone::Deck",
        "Zone::Hand { public: true }",
        "Zone::Hand { public: false }",
        "Zone::Field",
        "Zone::Graveyard",
        "Zone::Limbo { public: true }",
        "Zone::Limbo { public: false }",
        "Zone::CardSelection",
        "Zone::Casting",
        "Zone::Dust { public: true }",
        "Zone::Dust { public: false }",
    ];

    // Generate tests for moving from/to all ones excluding attachments.
    for card_ptr_bucket in &["None", "Some(0)", "Some(1)"] {
        // Option<Player>
        for base_card_type in &["BaseCard::Basic", "BaseCard::WithAttachment"] {
            // BaseCard
            for from_player in &["0", "1"] {
                for to_player in &["0", "1"] {
                    for from_zone in &zones {
                        for to_zone in &zones {
                            let stripped_name = identifier_ify_string(&format!(
                                "move_ptr_{}_base_{}_from_{}_{}_to_{}_{}",
                                card_ptr_bucket,
                                base_card_type,
                                from_player,
                                from_zone,
                                to_player,
                                to_zone
                            ));

                            let mut test = format!("
                                    #[test]
                                    fn test_{stripped_name}() {{
                                        let (mut tester, _owner_logs, player_logs) = make_tester();
                                        tester
                                            .apply(Some(0), &Action::Move {{
                                                card_ptr_bucket: {card_ptr_bucket},
                                                base_card_type: {base_card_type},
                                                from_player: {from_player},
                                                to_player: {to_player},
                                                from_zone: {from_zone},
                                                to_zone: {to_zone},
                                            }})
                                            .unwrap();

                                        println!(\"\n\nAll Logs:\");
                                        for card in player_logs.try_borrow_mut().unwrap()[0].clone() {{
                                            println!(\"{{}}\", card);
                                        }}
                                        println!(\"\n\");

                                        let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();
                                    ",
                                    stripped_name = stripped_name,
                                    card_ptr_bucket = card_ptr_bucket,
                                    base_card_type = base_card_type,
                                    from_player = from_player,
                                    from_zone = from_zone,
                                    to_player = to_player,
                                    to_zone = to_zone,
                                );

                            // If our BaseCard has an attachment, we'll see a MoveCard to attach it upon creation.
                            if base_card_type == &"BaseCard::WithAttachment" {
                                test += "
                                    let attach_event = actual_player_logs.next().expect(\"Expected attach event, got None.\");
                                    assert!(matches!(attach_event, CardEvent::MoveCard {
                                        to: ExactCardLocation {
                                            location: (Zone::Attachment{parent: Card::ID(_)}, _),
                                            ..
                                        },
                                        instance: Some(_),
                                        ..
                                    }), \"Base card has attachment, so expected attach event.\nGot {:#?}.\", attach_event);

                                    // ModifyCard of parent from attach callback.
                                    let modify_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::ModifyCard), got None.\");
                                    assert!(matches!(modify_event, CardEvent::ModifyCard {
                                        ..
                                    }));
                                ";
                            };

                            test += "
                                let move_to_start_zone_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                                assert!(matches!(move_to_start_zone_event, CardEvent::MoveCard{..}));
                            ";

                            // Event should fire if we moved to a different zone.
                            // Player 0 should see the card instance in every event involving it.
                            test += &format!("
                                        let move_to_end_zone_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                                        if {from_player} == 0 || {to_player} == 0 || ({to_zone}).is_public().unwrap() || ({from_zone}).is_public().unwrap() {{
                                            assert!(matches!(move_to_end_zone_event, CardEvent::MoveCard{{
                                                instance: Some(_),
                                                ..
                                            }}));
                                        }} else {{
                                            assert!(matches!(move_to_end_zone_event, CardEvent::MoveCard{{
                                                instance: None,
                                                ..
                                            }}));
                                        }}
                                    ",
                                    from_player = from_player,
                                    to_player = to_player,
                                    to_zone = to_zone,
                                    from_zone = from_zone
                                );

                            test += "\n}\n\n";
                            generated_tests.push_str(&test);
                        }
                    }
                }
            }
        }
    }

    // Generate tests for detaching into all zones.
    // Detach {
    //     parent_ptr_bucket: Zone,
    //     attachment_ptr_bucket: Option<Player>,
    //     to_player: Player,
    //     to_zone: Zone,
    // },
    for parent_zone in &zones {
        for attachment_ptr_bucket in &["None", "Some(0)", "Some(1)"] {
            for to_player in 0..2 {
                for to_zone in &zones {
                    let stripped_name = identifier_ify_string(&format!(
                        "detach_parent_in_{}_attachment_ptr_{}_to_{}_{}",
                        parent_zone, attachment_ptr_bucket, to_player, to_zone
                    ));
                    let mut test = format!(
                        "
                            #[test]
                            fn test_{stripped_name}() {{
                                let (mut tester, _owner_logs, player_logs) = make_tester();

                                tester
                                    .apply(Some(0), &Action::Detach {{
                                        parent_zone: {parent_zone},
                                        attachment_ptr_bucket: {attachment_ptr_bucket},
                                        to_player: {to_player},
                                        to_zone: {to_zone},
                                    }})
                                    .unwrap();

                                  println!(\"\n\nAll Logs:\");
                                  for card in player_logs.try_borrow_mut().unwrap()[0].clone() {{
                                      println!(\"{{:?}}\n\", card);
                                  }}
                                  println!(\"\n\");

                                let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();
                        ",
                        stripped_name = stripped_name,
                        parent_zone = parent_zone,
                        attachment_ptr_bucket = attachment_ptr_bucket,
                        to_player = to_player,
                        to_zone = to_zone,
                    );

                    // Our BaseCard has an attachment, so we'll see a MoveCard to attach it upon creation.
                    test += "
                        let attach_event = actual_player_logs.next().expect(\"Expected attach event, got None.\");
                        assert!(matches!(attach_event, CardEvent::MoveCard {
                            to: ExactCardLocation {
                                location: (Zone::Attachment{parent: Card::ID(_)}, _),
                                ..
                            },
                            instance: Some(_),
                            ..
                        }), \"Base card has attachment, so expected attach event.\nGot {:#?}.\", attach_event);

                        // ModifyCard of parent from attach callback.
                        let modify_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::ModifyCard), got None.\");
                        assert!(matches!(modify_event, CardEvent::ModifyCard {
                            ..
                        }));

                        let move_to_start_zone_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                        assert!(matches!(move_to_start_zone_event, CardEvent::MoveCard{..}));
                    ";

                    test += "
                        let move_to_end_zone_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                        assert!(matches!(move_to_end_zone_event, CardEvent::MoveCard {
                            instance: Some(_),
                            ..
                        }), \"Expected MoveCard to End Zone, got {:?}\", move_to_end_zone_event);
                    ";

                    // Player 0 should see the card instance in every event involving it.
                    test += "
                        // ModifyCard for parent being modified because of detach.
                        let modify_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::ModifyCard), got None.\");
                        assert!(matches!(modify_event, CardEvent::ModifyCard {
                            ..
                        }));
                    ";

                    test += "\n}\n\n";
                    generated_tests.push_str(&test);
                }
            }
        }
    }

    // Generate tests for attaching from all zones.
    // Attach {
    //     parent_base_card: BaseCard,
    //     parent_ptr_bucket: Option<Player>,
    //     parent_zone: Zone,
    //     card_ptr_bucket: Option<Player>,
    //     card_owner: Player,
    //     card_zone: Zone,
    // },
    for parent_base_card in &["BaseCard::Basic", "BaseCard::WithAttachment"] {
        for parent_ptr_bucket in &["None", "Some(0)", "Some(1)"] {
            for parent_zone in &zones {
                for card_ptr_bucket in &["None", "Some(0)", "Some(1)"] {
                    for card_owner in 0..2 {
                        for card_zone in &zones {
                            let stripped_name = identifier_ify_string(&format!(
                                "attach_to_{}_parent_ptr_{}_in_{}_card_ptr_{}_of_{}_in_{}",
                                parent_base_card,
                                parent_ptr_bucket,
                                parent_zone,
                                card_ptr_bucket,
                                card_owner,
                                card_zone,
                            ));

                            let mut test = format!(
                                "
                                    #[test]
                                    fn test_{stripped_name}() {{
                                        let (mut tester, _owner_logs, player_logs) = make_tester();

                                        tester
                                            .apply(Some(0), &Action::Attach {{
                                                parent_base_card: {parent_base_card},
                                                parent_ptr_bucket: {parent_ptr_bucket},
                                                parent_zone: {parent_zone},
                                                card_ptr_bucket: {card_ptr_bucket},
                                                card_owner: {card_owner},
                                                card_zone: {card_zone},
                                            }})
                                            .unwrap();


                                        println!(\"\n\nAll Logs:\");
                                        for card in player_logs.try_borrow_mut().unwrap()[0].clone() {{
                                            println!(\"{{}}\n\", card);
                                        }}
                                        println!(\"\n\");

                                        let mut actual_player_logs = player_logs.try_borrow_mut().unwrap()[0].clone().into_iter();
                                ",
                                stripped_name = stripped_name,
                                parent_base_card = parent_base_card,
                                parent_ptr_bucket = parent_ptr_bucket,
                                parent_zone = parent_zone,
                                card_ptr_bucket = card_ptr_bucket,
                                card_owner = card_owner,
                                card_zone = card_zone,
                            );
                            if parent_base_card == &"BaseCard::WithAttachment" {
                                // Our BaseCard has an attachment, so we'll see a MoveCard to attach it upon creation.
                                test += "
                                    let attach_event = actual_player_logs.next().expect(\"Expected attach event, got None.\");
                                    assert!(matches!(attach_event, CardEvent::MoveCard {
                                        to: ExactCardLocation {
                                            location: (Zone::Attachment{parent: Card::ID(_)}, _),
                                            ..
                                        },
                                        instance: Some(_),
                                        ..
                                    }), \"Base card has attachment, so expected attach event.\nGot {:#?}.\", attach_event);

                                    // ModifyCard of parent from attach callback.
                                    let modify_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::ModifyCard), got None.\");
                                    assert!(matches!(modify_event, CardEvent::ModifyCard {
                                        ..
                                    }));
                            ";
                            }
                            test += "
                                let move_parent_to_start_zone_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                                assert!(matches!(move_parent_to_start_zone_event, CardEvent::MoveCard{..}));
                            ";

                            test += "
                                let move_attach_to_start_zone_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                                assert!(matches!(move_attach_to_start_zone_event, CardEvent::MoveCard{..}));
                            ";

                            if parent_base_card == &"BaseCard::WithAttachment" {
                                test += "
                                    // Dust current attach
                                    let dust_current_attach = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                                    assert!(matches!(dust_current_attach, CardEvent::MoveCard {
                                        instance: Some(_),
                                        ..
                                    }), \"Expected MoveCard from Zone::Attach to Zone::Dust, got {:?}.\", dust_current_attach);

                                    // ModifyCard for parent being modified because of detach.
                                    let modify_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::ModifyCard), got None.\");
                                    assert!(matches!(modify_event, CardEvent::ModifyCard {
                                        ..
                                    }), \"Expected CardEvent::ModifyCard for parent because of child being detached, got {:?}\", modify_event);
                                ";
                            }
                            test += "
                                let attach_attachment_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::MoveCard), got None.\");
                                ";
                                
                                test += &format!("
                                let zone = {card_zone};
                                let is_mine = {card_owner} == 0;
                                let has_public_location = match zone {{
                                    Zone::Deck => true,
                                    Zone::Hand {{ .. }} => true,
                                    Zone::Field => true,
                                    Zone::Graveyard => true,
                                    Zone::Dust {{ .. }} => true,
                                    Zone::Attachment {{ .. }} => false,
                                    Zone::Limbo {{ public: false }} => false, // TODO this line should be removed, and the logging fixed
                                    Zone::Limbo {{ public }} => is_mine || public,
                                    Zone::Casting => true,
                                    Zone::CardSelection => true,
                                }};
                               ", card_zone = card_zone
                               , card_owner = card_owner);
                                test += "
                                if has_public_location {
                                    assert!(matches!(attach_attachment_event, CardEvent::MoveCard {
                                        instance: Some(_),
                                        from: CardLocation {
                                            location: Some(_),
                                            ..
                                        },
                                        ..
                                    }), \"Expected MoveCard to Zone::Attachment with Some(location), got {:?}.\", attach_attachment_event);
                                } else {
                                    assert!(matches!(attach_attachment_event, CardEvent::MoveCard {
                                        instance: Some(_),
                                        from: CardLocation {
                                            location: None,
                                            ..
                                        },
                                        ..
                                    }), \"Expected MoveCard to Zone::Attachment with location: None, got {:?}.\", attach_attachment_event);
                                }
                            ";

                            test += "
                                // Parent being modified because of new attach.
                                let modify_event = actual_player_logs.next().expect(\"Expected Some(CardEvent::ModifyCard), got None.\");
                                assert!(matches!(modify_event, CardEvent::ModifyCard {
                                    ..
                                }), \"Expected CardEvent::ModifyCard for parent because of new attach, got {:?}\", modify_event);
                            ";

                            test += "\n}\n\n";
                            generated_tests.push_str(&test);
                        }
                    }
                }
            }
        }
    }

    // Generate tests for copying cards.
    // CopyCard {
    //     card_ptr_bucket: Option<Player>, // 3
    //     base_card_type: BaseCard,        // 2
    //     deep: bool,                      // 2
    //     card_zone: Zone,
    // },
    for base_card_type in &["BaseCard::Basic", "BaseCard::WithAttachment"] {
        for card_ptr_bucket in &["None", "Some(0)", "Some(1)"] {
            for card_zone in &zones {
                for deep in &[false, true] {
                    let stripped_name = identifier_ify_string(&format!(
                        "copy_card_ptr_{}_card_{}_zone_{}_deep_{}",
                        card_ptr_bucket, base_card_type, card_zone, deep,
                    ));
                    generated_tests.push_str(&format!(
                        "
                            #[test]
                            fn test_{stripped_name}() {{
                              Tester::new(
                                  GameState::<State>::default(),
                                  [
                                      PlayerSecret::new(0, Default::default()),
                                      PlayerSecret::new(1, Default::default()),
                                  ],
                                  Default::default(),
                                  |_, _, _| {{}},
                                  |_, _| {{}},
                              )
                              .unwrap()
                              .apply(Some(0), &Action::CopyCard {{
                                  card_ptr_bucket: {card_ptr_bucket},
                                  base_card_type: {base_card_type},
                                    card_zone: {card_zone},
                                  deep: {deep},
                              }})
                              .unwrap();
                            }}

                            ",
                        stripped_name = stripped_name,
                        card_ptr_bucket = card_ptr_bucket,
                        base_card_type = base_card_type,
                        card_zone = card_zone,
                        deep = deep,
                    ))
                }
            }
        }
    }


    // Generate tests for resetting cards.
    // ResetCard {
    //     card_ptr_bucket: Option<Player>, // 3
    //     attachment_type: Option<BaseCard>,//2
    //     base_card_type: BaseCard,        // 2
    //     card_zone: Zone,
    // },
    for base_card_type in &["BaseCard::Basic", "BaseCard::WithAttachment"] {
        for attachment_type in &[
            "None",
            "Some(BaseCard::Basic)",
            "Some(BaseCard::Attachment)",
        ] {
            for card_ptr_bucket in &["None", "Some(0)", "Some(1)"] {
                for card_zone in &zones {
                    let stripped_name = identifier_ify_string(&format!(
                        "reset_card_ptr_{}_card_{}_attachment_{}_zone_{}",
                        card_ptr_bucket, base_card_type, attachment_type, card_zone,
                    ));
                    generated_tests.push_str(&format!(
                        "
                            #[test]
                            fn test_{stripped_name}() {{
                              Tester::new(
                                  GameState::<State>::default(),
                                  [
                                      PlayerSecret::new(0, Default::default()),
                                      PlayerSecret::new(1, Default::default()),
                                  ],
                                  Default::default(),
                                  |_, _, _| {{}},
                                  |_, _| {{}},
                              )
                              .unwrap()
                              .apply(Some(0), &Action::ResetCard {{
                                  card_ptr_bucket: {card_ptr_bucket},
                                  base_card_type: {base_card_type},
                                  attachment_type: {attachment_type},
                                  card_zone: {card_zone},
                              }})
                              .unwrap();
                            }}

                            ",
                        stripped_name = stripped_name,
                        card_ptr_bucket = card_ptr_bucket,
                        base_card_type = base_card_type,
                        attachment_type = attachment_type,
                        card_zone = card_zone,
                    ))
                }
            }
        }
    }

    // create file

    let test_file_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("generated_tests.rs");
    let mut file = File::create(test_file_path)?;
    file.write_all(&generated_tests.as_bytes())?;

    Ok(())
}

fn identifier_ify_string(string: &str) -> String {
    to_snake_case(
        &string
            .replace(&['(', ')', '{', '}'][..], "")
            .replace(&[' ', ':'][..], "_")
            .replace("__", "_")
            .replace("__", "_"),
    )
}
