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
