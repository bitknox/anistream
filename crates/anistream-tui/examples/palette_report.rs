//! Print the palette's measured contrast figures.
//!
//! Run with `cargo run -p anistream-ui --example palette_report`. Useful when tuning a
//! colour: the test suite only tells you pass or fail, this shows the margin.

use anistream_ui::theme::{
    color::{AA_NORMAL, Rgb},
    palette::{Palette, Role, Variant},
};

fn main() {
    let grounds: &[(&str, Variant, u32)] = &[
        ("black", Variant::Dark, 0x000000),
        ("tokyo-night", Variant::Dark, 0x1A1B26),
        ("catppuccin", Variant::Dark, 0x1E1E2E),
        ("gruvbox", Variant::Dark, 0x282828),
        ("nord", Variant::Dark, 0x2E3440),
        ("white", Variant::Light, 0xFFFFFF),
        ("solarized-lt", Variant::Light, 0xFDF6E3),
        ("immersive", Variant::Immersive, 0x161A2E),
    ];

    print!("{:<14}", "ground");
    for role in Role::ALL {
        print!("{:>11}", role.name());
    }
    println!();
    println!("{}", "─".repeat(14 + 11 * Role::ALL.len()));

    for &(name, variant, hex) in grounds {
        let p = Palette::for_ground(variant, Rgb::from_hex(hex));
        print!("{name:<14}");
        for role in Role::ALL {
            let ratio = p.worst_case_contrast(role);
            let flag = if role.is_text() && ratio < AA_NORMAL { "!" } else { " " };
            print!("{ratio:>10.2}{flag}");
        }
        println!();
    }

    println!();
    println!(
        "text roles measured against each variant's worst supported ground; \
         hairlines against the actual ground (they are derived from it)"
    );
    println!("AA floor for normal text = {AA_NORMAL}:1");
}
