# Mollie

A scripting language designed to describe the interface in the Meralus project. However, it can also be used separately.

Mollie is based on a fairly strict type system. This is intentional, as part of the design. You may also notice that the syntax is largely the same as Rust. Nothing strange about that, I just really like that language.

### Types
- **Primitive** types, passed as **values**: these include integers and floating-point numbers, as well as boolean values.
- **Complex** types, passed as **references**: types that cannot be copied, i.e., strings, structures, components and functions (including native ones).

