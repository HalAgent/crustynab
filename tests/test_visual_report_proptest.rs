use crustynab::visual_report::{CURRENCY, darken_hex, format_currency};
use proptest::prelude::*;

fn is_valid_hex_color(value: &str) -> bool {
    value.starts_with('#')
        && value.len() == 7
        && value[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn prop_format_currency_zero_behavior(
        value in -1_000_000.0f64..1_000_000.0f64,
        show_zero in any::<bool>(),
    ) {
        let rounded = (value * 100.0).round() / 100.0;
        let formatted = format_currency(value, show_zero);

        if rounded == 0.0 && !show_zero {
            prop_assert!(formatted.is_empty());
            return Ok(());
        }

        prop_assert!(!formatted.is_empty());
        prop_assert!(formatted.contains(CURRENCY));

        if rounded < 0.0 {
            prop_assert!(formatted.starts_with('-'));
        }
    }

    #[test]
    fn prop_darken_hex_preserves_format(
        red in any::<u8>(),
        green in any::<u8>(),
        blue in any::<u8>(),
        factor in 0.0f64..=1.0f64,
    ) {
        let color = format!("#{red:02x}{green:02x}{blue:02x}");
        let darkened = darken_hex(&color, factor);

        prop_assert!(darkened.starts_with('#'));
        prop_assert_eq!(darkened.len(), 7);

        let dark_red = u8::from_str_radix(&darkened[1..3], 16).expect("hex");
        let dark_green = u8::from_str_radix(&darkened[3..5], 16).expect("hex");
        let dark_blue = u8::from_str_radix(&darkened[5..7], 16).expect("hex");

        prop_assert!(dark_red <= red);
        prop_assert!(dark_green <= green);
        prop_assert!(dark_blue <= blue);
    }

    #[test]
    fn prop_darken_hex_invalid_passthrough(value in "[ -~]{0,12}") {
        prop_assume!(!is_valid_hex_color(&value));
        prop_assert_eq!(darken_hex(&value, 0.85), value);
    }
}
