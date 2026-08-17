//! **Units are exact, and they are never inferred.**
//!
//! Every unit either format can state is a *rational* multiple of a millimetre — `1 in = 127/5 mm`
//! by the international definition, and every CSS absolute unit is a rational fraction of an inch
//! (`1 px = 1/96 in`, so `1 mm = 480/127 px`). So conversion costs nothing and there is never a
//! reason to guess.
//!
//! Guessing is the failure worth designing against: a file read in the wrong unit produces a part
//! that is off by 25.4×, and it will look entirely plausible in a viewer. A file that declares no
//! unit is therefore **refused** unless the caller supplies one, and the unit found, the unit
//! produced and the exact factor between them all appear in the import report.
//!
//! ```
//! use interchange::unit::Unit;
//! use lattice::{Bignum, Rat};
//!
//! // The conversion is a rational, not a rounded constant.
//! assert_eq!(Unit::Inch.factor_to::<Bignum>(Unit::Millimetre), Rat::new(127, 5));
//! assert_eq!(Unit::Millimetre.factor_to::<Bignum>(Unit::Pixel), Rat::new(480, 127));
//! ```

use lattice::{Backend, Rat};

/// A length unit a DXF or SVG file can name.
///
/// The kernel's own unit is the **millimetre** (the §14 BONDED shear budget is quoted there), so
/// every factor here is expressed against it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// 10⁻⁶ m.
    Micrometre,
    /// 10⁻³ m — the kernel's own unit.
    Millimetre,
    /// 10⁻² m.
    Centimetre,
    /// 10⁻¹ m.
    Decimetre,
    /// The metre.
    Metre,
    /// 10³ m.
    Kilometre,
    /// A thou — `1/1000 in`, the unit a flex fabricator quotes trace widths in.
    Mil,
    /// `127/5 mm`, exactly (the international inch).
    Inch,
    /// 12 inches.
    Foot,
    /// 36 inches.
    Yard,
    /// The typographic point, `1/72 in`.
    Point,
    /// The pica, `1/6 in`.
    Pica,
    /// The CSS reference pixel, `1/96 in`.
    Pixel,
}

impl Unit {
    /// This unit in millimetres, exactly — `(numerator, denominator)`.
    const fn mm_ratio(self) -> (i128, i128) {
        match self {
            Unit::Micrometre => (1, 1000),
            Unit::Millimetre => (1, 1),
            Unit::Centimetre => (10, 1),
            Unit::Decimetre => (100, 1),
            Unit::Metre => (1000, 1),
            Unit::Kilometre => (1_000_000, 1),
            Unit::Mil => (127, 5000),
            Unit::Inch => (127, 5),
            Unit::Foot => (1524, 5),
            Unit::Yard => (4572, 5),
            Unit::Point => (127, 360),
            Unit::Pica => (127, 30),
            Unit::Pixel => (127, 480),
        }
    }

    /// The exact factor a length in `self` is multiplied by to become a length in `target`.
    pub fn factor_to<B: Backend>(self, target: Unit) -> Rat<B> {
        let (an, ad) = self.mm_ratio();
        let (bn, bd) = target.mm_ratio();
        // (an/ad) / (bn/bd) = (an·bd) / (ad·bn) — every product is small, so this never widens.
        Rat::new(an, ad).div(&Rat::new(bn, bd))
    }

    /// The unit a DXF `$INSUNITS` header value names.
    ///
    /// `0` is *unitless*, which is a declaration that the file does not say — so it maps to `None`
    /// and the reader refuses unless the caller named one. Values outside the supported set (miles,
    /// ångströms, astronomical units) also return `None`: refusing an exotic unit is better than
    /// silently reading it as the default.
    pub fn from_dxf_insunits(code: i64) -> Option<Unit> {
        Some(match code {
            1 => Unit::Inch,
            2 => Unit::Foot,
            4 => Unit::Millimetre,
            5 => Unit::Centimetre,
            6 => Unit::Metre,
            7 => Unit::Kilometre,
            9 => Unit::Mil,
            10 => Unit::Yard,
            13 => Unit::Micrometre,
            14 => Unit::Decimetre,
            _ => return None,
        })
    }

    /// The unit a CSS/SVG length suffix names (`"mm"`, `"in"`, `"px"`, …).
    ///
    /// An empty suffix is SVG's "user units", which are pixels **only** once a `viewBox` has been
    /// resolved — so it is deliberately not accepted here; the caller decides what a user unit
    /// means. Relative units (`em`, `ex`, `%`) have no absolute length and return `None`.
    pub fn from_css_suffix(suffix: &str) -> Option<Unit> {
        Some(match suffix {
            "mm" => Unit::Millimetre,
            "cm" => Unit::Centimetre,
            "in" => Unit::Inch,
            "pt" => Unit::Point,
            "pc" => Unit::Pica,
            "px" => Unit::Pixel,
            _ => return None,
        })
    }

    /// The short name used in reports and written into files.
    pub fn name(self) -> &'static str {
        match self {
            Unit::Micrometre => "um",
            Unit::Millimetre => "mm",
            Unit::Centimetre => "cm",
            Unit::Decimetre => "dm",
            Unit::Metre => "m",
            Unit::Kilometre => "km",
            Unit::Mil => "mil",
            Unit::Inch => "in",
            Unit::Foot => "ft",
            Unit::Yard => "yd",
            Unit::Point => "pt",
            Unit::Pica => "pc",
            Unit::Pixel => "px",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    /// The inch is *defined* as 25.4 mm, so the conversion is a rational and the round trip is the
    /// identity — not "the identity to within a tolerance".
    #[test]
    fn conversion_is_exact_and_round_trips() {
        assert_eq!(
            Unit::Inch.factor_to::<Bignum>(Unit::Millimetre),
            Q::new(127, 5)
        );
        assert_eq!(
            Unit::Millimetre.factor_to::<Bignum>(Unit::Inch),
            Q::new(5, 127)
        );
        for u in [Unit::Inch, Unit::Mil, Unit::Point, Unit::Pixel, Unit::Metre] {
            let there = u.factor_to::<Bignum>(Unit::Millimetre);
            let back = Unit::Millimetre.factor_to::<Bignum>(u);
            assert_eq!(there.mul(&back), Q::from_i128(1), "{} round trip", u.name());
        }
    }

    /// The CSS ladder, which is where a plausible-looking 25.4× error would come from.
    #[test]
    fn the_css_ladder_is_rational_all_the_way_down() {
        // 96 px = 1 in = 72 pt = 6 pc.
        assert_eq!(
            Unit::Pixel
                .factor_to::<Bignum>(Unit::Inch)
                .mul(&Q::from_i128(96)),
            Q::from_i128(1)
        );
        assert_eq!(
            Unit::Point
                .factor_to::<Bignum>(Unit::Inch)
                .mul(&Q::from_i128(72)),
            Q::from_i128(1)
        );
        assert_eq!(
            Unit::Pica
                .factor_to::<Bignum>(Unit::Inch)
                .mul(&Q::from_i128(6)),
            Q::from_i128(1)
        );
        // …and the one every SVG reader needs: 1 mm is exactly 480/127 px.
        assert_eq!(
            Unit::Millimetre.factor_to::<Bignum>(Unit::Pixel),
            Q::new(480, 127)
        );
    }

    /// "Unitless" is a declaration that the file does not say, and it must not resolve to a
    /// default — that is precisely the 25.4× part.
    #[test]
    fn unitless_and_exotic_units_do_not_resolve() {
        assert_eq!(Unit::from_dxf_insunits(0), None, "unitless is not a unit");
        assert_eq!(Unit::from_dxf_insunits(3), None, "miles");
        assert_eq!(Unit::from_dxf_insunits(18), None, "astronomical units");
        assert_eq!(Unit::from_dxf_insunits(4), Some(Unit::Millimetre));
        assert_eq!(
            Unit::from_css_suffix(""),
            None,
            "user units are the caller's call"
        );
        assert_eq!(Unit::from_css_suffix("%"), None);
        assert_eq!(Unit::from_css_suffix("em"), None);
        assert_eq!(Unit::from_css_suffix("mm"), Some(Unit::Millimetre));
    }
}
