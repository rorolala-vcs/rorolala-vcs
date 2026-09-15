# rorolala-utils-configure-macros

The `Configure` derive, an implementation detail of `rorolala-utils-configure`, which
re-exports it — depend on that crate instead.

The derive declares that a type is the contents of a configuration file. The trait's
methods have default bodies, so the derive adds nothing but the opt-in: being a
configuration is a decision the type makes, rather than something a blanket
implementation would make for every type serde can already handle.
