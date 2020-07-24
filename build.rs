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
                              )
                              .unwrap()
                              .apply(Some(0), &Action::Move {{
                                  card_ptr_bucket: {card_ptr_bucket},
                                  base_card_type: {base_card_type},
                                  from_player: {from_player},
                                  to_player: {to_player},
                                  from_zone: {from_zone},
                                  to_zone: {to_zone},
                              }})
                              .unwrap();
                            }}

                            ",
                                stripped_name = stripped_name,
                                card_ptr_bucket = card_ptr_bucket,
                                base_card_type = base_card_type,
                                from_player = from_player,
                                from_zone = from_zone,
                                to_player = to_player,
                                to_zone = to_zone,
                            ))
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
                          )
                          .unwrap()
                          .apply(Some(0), &Action::Detach {{
                              parent_zone: {parent_zone},
                              attachment_ptr_bucket: {attachment_ptr_bucket},
                              to_player: {to_player},
                              to_zone: {to_zone},
                          }})
                          .unwrap();
                        }}

                        ",
                        stripped_name = stripped_name,
                        parent_zone = parent_zone,
                        attachment_ptr_bucket = attachment_ptr_bucket,
                        to_player = to_player,
                        to_zone = to_zone,
                    ))
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
                          )
                          .unwrap()
                          .apply(Some(0), &Action::Attach {{
                              parent_base_card: {parent_base_card},
                              parent_ptr_bucket: {parent_ptr_bucket},
                              parent_zone: {parent_zone},
                              card_ptr_bucket: {card_ptr_bucket},
                              card_owner: {card_owner},
                              card_zone: {card_zone},
                          }})
                          .unwrap();
                        }}

                        ",
                                stripped_name = stripped_name,
                                parent_base_card = parent_base_card,
                                parent_ptr_bucket = parent_ptr_bucket,
                                parent_zone = parent_zone,
                                card_ptr_bucket = card_ptr_bucket,
                                card_owner = card_owner,
                                card_zone = card_zone,
                            ))
                        }
                    }
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
