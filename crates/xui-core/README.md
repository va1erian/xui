# xui-core

The backend-agnostic core of [xui](../xui): the portable front layer every
backend shares.

It contains pixel geometry, typed length units (`Dip`/`Px`), colour, the pure
layout arithmetic (`Dock`/`Stack`/`Anchor`/`Insets`), the semantic theme tokens,
the input vocabulary, the accessibility tree model, the **portable widget
layer**, the `App`/`Ui` runtime, and the `Backend` contract a platform
implements.

It has no platform dependency and no `unsafe`, so it compiles on every target.
See the [workspace README](../../README.md),
[Architecture](../../docs/architecture.md) and [Widgets](../../docs/widgets.md).
