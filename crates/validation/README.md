# ✅ validation

Shared validation functions, predicates, and custom validators for Rust applications using [`validator`](https://github.com/Keats/validator).

---

## ✨ Features

- 🆔 **UUID Validation**: Validates UUID v7 strings and binary representations.
- 🏢 **Tenant & Slug Checks**: Validates tenant keys, organization identifiers, and URL slug formats.
- 🔢 **TOTP & MFA Checks**: Validates numeric TOTP codes, recovery codes, and base32 secret structures.
- ⚡ **Zero-Overhead Integration**: Directly compatible with `#[validate(custom = "...")]` attributes.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `validation` to your `Cargo.toml`:

```toml
[dependencies]
validation = "0.1"
validator = { version = "0.21", features = ["derive"] }
```

### 2. Usage Example

```rust
use validator::Validate;
use validation::validate_slug;

#[derive(Validate)]
struct CreateOrganizationRequest {
    #[validate(custom(function = "validate_slug"))]
    slug: String,
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
