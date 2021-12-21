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
                                        test_move(
                                            {card_ptr_bucket},
                                            {base_card_type},
                                            {from_player},
                                            {from_zone},
                                            {to_player},
                                            {to_zone},
                                        );
                                    }}",
                                stripped_name = stripped_name,
                                card_ptr_bucket = card_ptr_bucket,
                                base_card_type = base_card_type,
                                from_player = from_player,
                                from_zone = from_zone,
                                to_player = to_player,
                                to_zone = to_zone,
                            ));
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
                                test_detach(
                                    {parent_zone},
                                    {attachment_ptr_bucket},
                                    {to_player},
                                    {to_zone},
                                );
                            }}
                        ",
                        stripped_name = stripped_name,
                        parent_zone = parent_zone,
                        attachment_ptr_bucket = attachment_ptr_bucket,
                        to_player = to_player,
                        to_zone = to_zone,
                    ));
                }
            }
        }
    }

    // Generate tests for attaching from all zones except already attached.
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
                                        test_attach(
                                            {parent_base_card},
                                            {parent_ptr_bucket},
                                            {parent_zone},
                                            {card_ptr_bucket},
                                            {card_owner},
                                            {card_zone},
                                        );
                                    }}
                                ",
                                stripped_name = stripped_name,
                                parent_base_card = parent_base_card,
                                parent_ptr_bucket = parent_ptr_bucket,
                                parent_zone = parent_zone,
                                card_ptr_bucket = card_ptr_bucket,
                                card_owner = card_owner,
                                card_zone = card_zone,
                            ));
                        }
                    }
                }
            }
        }
    }

    // Generate tests for moving an attachment from one parent to another.
    // AttachFromAttached {
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
                                "attach_from_attached_to_{}_parent_ptr_{}_in_{}_card_ptr_{}_of_{}_in_{}",
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
                                        test_attach_from_attached(
                                            {parent_base_card},
                                            {parent_ptr_bucket},
                                            {parent_zone},
                                            {card_ptr_bucket},
                                            {card_owner},
                                            {card_zone}
                                        );
                                    }}
                                ",
                                stripped_name = stripped_name,
                                parent_base_card = parent_base_card,
                                parent_ptr_bucket = parent_ptr_bucket,
                                parent_zone = parent_zone,
                                card_ptr_bucket = card_ptr_bucket,
                                card_owner = card_owner,
                                card_zone = card_zone,
                            ));
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
    //     card_zone: Zone,
    //     deep: bool,                      // 2
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
                              test_copy_card(
                                 {card_ptr_bucket},
                                 {base_card_type},
                                 {card_zone},
                                 {deep},
                              );
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
                              test_reset_card(
                                  {card_ptr_bucket},
                                  {attachment_type},
                                  {base_card_type},
                                  {card_zone},
                              );
                            }}

                            ",
                        stripped_name = stripped_name,
                        card_ptr_bucket = card_ptr_bucket,
                        attachment_type = attachment_type,
                        base_card_type = base_card_type,
                        card_zone = card_zone,
                    ))
                }
            }
        }
    }

    // create file

    let test_file_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("generated_tests.rs");
    let mut file = File::create(test_file_path)?;
    file.write_all(generated_tests.as_bytes())?;

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
