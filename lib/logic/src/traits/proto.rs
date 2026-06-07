//! Wire-format proto conversion traits.

/// Build the wire-format proto packet `P` from a borrowed snapshot of `Self`.
///
/// Kept distinct from `From<&Self> for P` because:
///
///   * the project already uses `From` impls heavily, coherence rules
///     forbid a blanket `impl<T> From<&T> for SomeProto`,
///   * some conversions need auxiliary context (asset tables, locale, ...);
///     see [`ToProtoWith`] for that case,
///   * we still want the ergonomic, no-prefix `.to_proto()` call.
///
/// A blanket impl forwards to `Into` so every existing `From<&T> for P` is
/// automatically a `ToProto<P>` without extra boilerplate.
pub trait ToProto<P> {
    fn to_proto(&self) -> P;
}

impl<T, P> ToProto<P> for T
where
    for<'a> &'a T: Into<P>,
{
    #[inline]
    fn to_proto(&self) -> P {
        self.into()
    }
}

/// Context-aware variant, conversions that need static asset tables.
pub trait ToProtoWith<P, Ctx: ?Sized> {
    fn to_proto_with(&self, ctx: &Ctx) -> P;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple domain value.
    #[derive(Debug, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }

    /// A simple proto representation.
    #[derive(Debug, PartialEq)]
    struct ProtoPoint {
        x: i32,
        y: i32,
    }

    impl From<&Point> for ProtoPoint {
        fn from(p: &Point) -> Self {
            ProtoPoint { x: p.x, y: p.y }
        }
    }

    #[test]
    fn to_proto_uses_into_impl() {
        let point = Point { x: 10, y: 20 };
        let proto: ProtoPoint = point.to_proto();
        assert_eq!(proto, ProtoPoint { x: 10, y: 20 });
    }

    #[test]
    fn to_proto_matches_from() {
        let point = Point { x: -5, y: 42 };
        let via_to_proto: ProtoPoint = point.to_proto();
        let via_from: ProtoPoint = (&point).into();
        assert_eq!(via_to_proto, via_from);
    }

    /// Context-aware conversion test.
    struct ScaleContext {
        factor: i32,
    }

    struct ScaledProtoPoint {
        x: i32,
        y: i32,
    }

    impl ToProtoWith<ScaledProtoPoint, ScaleContext> for Point {
        fn to_proto_with(&self, ctx: &ScaleContext) -> ScaledProtoPoint {
            ScaledProtoPoint {
                x: self.x * ctx.factor,
                y: self.y * ctx.factor,
            }
        }
    }

    #[test]
    fn to_proto_with_applies_context() {
        let point = Point { x: 3, y: 4 };
        let ctx = ScaleContext { factor: 2 };
        let scaled = point.to_proto_with(&ctx);
        assert_eq!(scaled.x, 6);
        assert_eq!(scaled.y, 8);
    }
}
