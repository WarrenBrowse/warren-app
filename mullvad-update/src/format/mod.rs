//! This module includes all that is needed for the (de)serialization of Mullvad version metadata.
//! This includes ensuring authenticity and integrity of version metadata, and rejecting expired
//! metadata. There are also tools for producing new versions.
//!
//! Fundamentally, a version object is a JSON object with a `signed` key and a `signature` key.
//! `signature` contains a public key and an ed25519 signature of `signed` in canonical JSON form.
//! `signed` also contains an `expires` field, which is a timestamp indicating when the object
//! expires.
//!
//! For the deserializer to succeed in deserializing a file, it must verify that the canonicalized
//! form of `signed` is in fact signed by key/signature in `signature`. It also reads the `expires`
//! and rejects the file if it has expired.

pub mod architecture;
pub mod deserializer;
pub mod installer;
pub mod key;
pub mod release;
pub mod response;
#[cfg(feature = "sign")]
pub mod serializer;

pub use architecture::Architecture;

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use crate::format::release::Release;
    use crate::version::rollout::Rollout;

    #[test]
    fn changelog_translations_are_optional_and_omitted_when_empty() {
        // The signature covers the canonical JSON of `signed`, so a field that
        // always serialized would change the bytes of every manifest that has
        // no translations at all. It must stay absent unless populated.
        let untranslated = serde_json::to_value(Release {
            version: "2024.1".parse().unwrap(),
            changelog: "notes".to_owned(),
            changelog_translations: BTreeMap::new(),
            installers: vec![],
            rollout: Rollout::complete(),
        })
        .unwrap();
        assert!(untranslated.get("changelog_translations").is_none());

        // A manifest published before the field existed still deserializes.
        let legacy: Release = serde_json::from_value(serde_json::json!({
            "version": "2024.1",
            "changelog": "notes",
            "installers": [],
        }))
        .expect("a manifest without translations must still parse");
        assert!(legacy.changelog_translations.is_empty());

        let translated: Release = serde_json::from_value(serde_json::json!({
            "version": "2024.1",
            "changelog": "notes",
            "changelog_translations": { "fr": "notes en francais" },
            "installers": [],
        }))
        .expect("a manifest with translations must parse");
        assert_eq!(
            translated
                .changelog_translations
                .get("fr")
                .map(String::as_str),
            Some("notes en francais")
        );
    }

    #[test]
    fn test_default_rollout_serialize() {
        // rollout should not be serialized if equal to default value
        let serialized = serde_json::to_value(Release {
            version: "2024.1".parse().unwrap(),
            changelog: "".to_owned(),
            changelog_translations: BTreeMap::new(),
            installers: vec![],
            rollout: Rollout::complete(),
        })
        .unwrap();

        assert_eq!(
            serialized,
            serde_json::json!({
                "version": "2024.1",
                "changelog": "",
                "installers": [],
            })
        );

        // rollout *should* be serialized if not equal to default value
        let rollout = Rollout::try_from(0.99).unwrap();
        let serialized = serde_json::to_value(Release {
            version: "2024.1".parse().unwrap(),
            changelog: "".to_owned(),
            changelog_translations: BTreeMap::new(),
            installers: vec![],
            rollout,
        })
        .unwrap();

        assert_eq!(
            serialized,
            serde_json::json!({
                "version": "2024.1",
                "changelog": "",
                "installers": [],
                "rollout": rollout,
            })
        );
    }
}
