# Markdown view

A paragraph with **bold**, *italic*, ~~struck~~ and `inline code`, a
[link to the next page](next.md) and a [jump to the table](#a-table).
Long words wrap: https://example.com/a/very/long/address/that/does/not/fit/on/one/line/at/all

## Lists

- First item
- Second item, long enough to wrap onto a second line so the indent can be checked
  - Nested item
- [x] A done task
- [ ] A task left to do

1. One
2. Two
3. Three

> A quote, drawn weak with a bar.
>
> > A nested quote.

## Code

```rust
fn main() {
	println!("Hello, {}!", "world");
}
```

---

## A table

| Message | What it does | Count |
| :--- | :---: | ---: |
| `say:` | Shows text | 1 |
| `move:` | Moves the **entity** to a point | 22 |

![A picture](pic.png) is drawn once the app supplies its pixels, and shows its alt text until then.
