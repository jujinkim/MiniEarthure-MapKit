//! Full-audit strict pass: retain only keys of open objects, never a Value tree.
use super::*;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

pub(crate) fn document(bytes: &[u8], mut check: impl FnMut() -> Result<()>) -> Result<MapDocument> {
    struct State<'a> {
        check: &'a mut dyn FnMut() -> Result<()>,
        count: usize,
        failure: Option<Error>,
    }
    struct Scan<'a, 'b>(&'a mut State<'b>);
    impl<'de> DeserializeSeed<'de> for Scan<'_, '_> {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(
            self,
            d: D,
        ) -> std::result::Result<(), D::Error> {
            if self.0.count.is_multiple_of(64) {
                if let Err(error) = (self.0.check)() {
                    self.0.failure = Some(error);
                    return Err(serde::de::Error::custom("audit JSON scan cancelled"));
                }
            }
            self.0.count += 1;
            d.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Scan<'_, '_> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("integer JSON without duplicate keys")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
            let mut keys = BTreeSet::new();
            while let Some(key) = map.next_key::<String>()? {
                if !keys.insert(key) {
                    return Err(serde::de::Error::custom("duplicate JSON key"));
                }
                map.next_value_seed(Scan(self.0))?;
            }
            Ok(())
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
            while seq.next_element_seed(Scan(self.0))?.is_some() {}
            Ok(())
        }
        fn visit_bool<E: serde::de::Error>(self, _: bool) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<(), E> {
            Err(E::custom("only integer JSON numbers supported"))
        }
        fn visit_str<E: serde::de::Error>(self, _: &str) -> std::result::Result<(), E> {
            Ok(())
        }
        fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<(), E> {
            Ok(())
        }
    }
    check()?;
    let mut state = State {
        check: &mut check,
        count: 0,
        failure: None,
    };
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    if let Err(error) = Scan(&mut state)
        .deserialize(&mut decoder)
        .and_then(|()| decoder.end())
    {
        return Err(state
            .failure
            .unwrap_or_else(|| super::error("E_JSON", error.to_string())));
    }
    // All key sets and the strict decoder scratch are gone before typed parsing.
    drop(decoder);
    check()?;
    let document = serde_json::from_slice(bytes).map_err(|e| error("E_JSON", e.to_string()))?;
    check()?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Vec<u8> {
        include_bytes!("../../../examples/roads/document.json").to_vec()
    }
    #[test]
    fn strict_direct_parse_matches_tree_and_rejects_nested_aliases() {
        let bytes = source();
        let direct = document(&bytes, || Ok(())).unwrap();
        assert_eq!(
            canonical(&direct).unwrap(),
            canonical(&json::<MapDocument>(&bytes).unwrap()).unwrap()
        );
        let text = String::from_utf8(bytes).unwrap();
        for invalid in [
            format!("{{\"map_id\":\"duplicate\",{}", &text[1..]),
            text.replacen("\"map_id\"", "\"map_id\":\"duplicate\",\"map_\\u0069d\"", 1),
            text.replacen(
                "\"bounds\":",
                "\"bounds\":{\"min\":[0,0],\"m\\u0069n\":[0,0],\"max\":[1,1]},\"bounds\":",
                1,
            ),
            format!("{text} null"),
            text.replacen("\"revision\":", "\"revision\":1.0,\"unknown\":", 1),
            text.replacen("\"revision\":", "\"revision\":1e0,\"unknown\":", 1),
            text.replacen("\"map_id\"", "\"unknown_field\"", 1),
        ] {
            assert!(json::<MapDocument>(invalid.as_bytes()).is_err());
            assert!(document(invalid.as_bytes(), || Ok(())).is_err());
        }
        let nested = format!("{}0{}", "[".repeat(129), "]".repeat(129));
        assert!(document(nested.as_bytes(), || Ok(())).is_err());
    }
    #[test]
    fn strict_scan_cancels_during_large_array_before_typed_decode() {
        let bytes = format!("[{}]", vec!["0"; 1024].join(","));
        let mut checks = 0;
        let failure = document(bytes.as_bytes(), || {
            checks += 1;
            if checks == 4 {
                Err(error("E_CANCELLED", "test scan"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(failure.code, "E_CANCELLED");
        assert_eq!(checks, 4);
    }
}
