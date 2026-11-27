// rec.rs — `Rec<V>`, a TypeScript-only stand-in that makes ts-rs write a map as `Record<string, V>`.
// A reader would get wrong that it is never built: `ts(as = "Rec<V>")` names it. It is `song/src/rec.rs`'s twin, since this crate has no `song`.

// ts-rs's own contract for a type that is no declaration: asking for one is a bug in the exporter.
#![allow(clippy::panic)]

use std::marker::PhantomData;
use ts_rs::{TS, TypeVisitor};

pub struct Rec<V>(PhantomData<V>);

impl<V: TS + 'static> TS for Rec<V> {
    type WithoutGenerics = Rec<ts_rs::Dummy>;
    type OptionInnerType = Self;

    fn name() -> String {
        format!("Record<string, {}>", V::name())
    }

    fn inline() -> String {
        Self::name()
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        V::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        V::visit_generics(v);
        v.visit::<V>();
    }
}
