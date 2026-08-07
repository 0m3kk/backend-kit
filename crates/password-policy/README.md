# password-policy

Production-grade, customizable, type-safe password policy enforcement, strength estimation, context-aware validation, and policy-compliant password generation crate for `backend-kit`.

## Features

- **Standard Presets**:
  - `PasswordPolicy::nist()`: OWASP & NIST SP 800-63B compliant (focuses on length 8–128, blocklists, user context, allowing spaces).
  - `PasswordPolicy::owasp()`: OWASP baseline standards (min length 10, required character diversity).
  - `PasswordPolicy::strict()`: Strict enterprise security (min length 14, character class minimums, bit entropy threshold, repeat/sequence checks).
  - `PasswordPolicy::builder()`: Fluent API for custom policy configuration.
- **Context-Aware Validation (`UserContext`)**:
  - Prevents users from employing personal details (username, email prefix, names, company name) in candidate passwords.
- **Entropy & Strength Estimation**:
  - Calculates Shannon character-set entropy in bits ($E = L \times \log_2(R)$) and categorizes strength (`Weak`, `Medium`, `Strong`, `VeryStrong`).
- **Pattern & Sequence Detection**:
  - Rejects consecutive repetitive characters (e.g. `aaaa`) and sequential alphanumeric / keyboard row sequences (e.g. `12345`, `qwerty`).
- **Built-in & Custom Blocklists**:
  - Embedded list of top weak/common passwords + support for custom forbidden word lists.
- **Cryptographically Secure Generator (`PasswordGenerator`)**:
  - Generates policy-compliant random passwords with guaranteed rule satisfaction.
- **Async Breach Checking (`hibp` feature)**:
  - Optional integration with Have I Been Pwned API using k-Anonymity SHA-1 prefix matching (`AsyncBreachChecker` & `HibpClient`).

## Feature Flags

| Feature     | Description                                                                                 | Default |
| :---------- | :------------------------------------------------------------------------------------------ | :------ |
| `generator` | Cryptographically secure random `PasswordGenerator` meeting policy constraints.             | **Yes** |
| `hibp`      | Have I Been Pwned k-Anonymity API client (`HibpClient`) for checking compromised passwords. | No      |

## Usage

Add `password-policy` to your `Cargo.toml`:

```toml
[dependencies]
password-policy = { path = "crates/password-policy" }
```

### 1. Basic Policy Validation

```rust
use password_policy::{PasswordPolicy, PolicyError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy = PasswordPolicy::default();

    // Validate a compliant password
    policy.validate("P@ssw0rd123!")?;

    // Inspect validation error details
    match policy.validate("short") {
        Ok(_) => println!("Valid password"),
        Err(PolicyError::Violations(violations)) => {
            for v in violations {
                println!("Policy violation: {:?}", v);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
```

### 2. User-Context Aware Validation & Detailed Auditing

```rust
use password_policy::{PasswordPolicy, UserContext};

fn main() {
    let policy = PasswordPolicy::owasp();
    let user_ctx = UserContext::new()
        .with_username("johndoe")
        .with_email("john.doe@company.com")
        .with_name("John", "Doe");

    // Perform detailed audit
    let report = policy.audit_with_context("JohnDoe2026!", &user_ctx);

    println!("Is Valid: {}", report.is_valid());
    println!("Entropy: {:.1} bits", report.entropy_bits);
    println!("Strength: {:?}", report.strength);

    if !report.is_valid {
        println!("Violations: {:?}", report.violations);
    }
}
```

### 3. Fluent Policy Builder

```rust
use password_policy::{PasswordPolicy, PolicyViolation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy = PasswordPolicy::builder()
        .min_length(12)
        .max_length(64)
        .min_uppercase(1)
        .min_lowercase(1)
        .min_digits(1)
        .min_special(1)
        .min_entropy_bits(45.0)
        .max_consecutive_repeat(3)
        .prohibit_sequential(true)
        .blocklist(["companysecret", "admin2026"])
        .build()?;

    let report = policy.audit("CompanySecret123!");
    assert!(!report.is_valid());

    Ok(())
}
```

### 4. Cryptographically Secure Password Generation

```rust
use password_policy::{PasswordGenerator, PasswordPolicy};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy = PasswordPolicy::strict();
    let generator = PasswordGenerator::new();

    // Generate a 20-character password compliant with strict enterprise policy
    let password = generator.generate_with_length(&policy, 20)?;
    println!("Generated Password: {}", password);

    assert!(policy.validate(&password).is_ok());
    Ok(())
}
```

### 5. Have I Been Pwned (HIBP) Breach Checking (`hibp` feature)

```rust
#[cfg(feature = "hibp")]
use password_policy::{AsyncBreachChecker, HibpClient};

#[cfg(feature = "hibp")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hibp = HibpClient::new();

    if let Some(breach_count) = hibp.check_breach("password123").await? {
        println!("Password found in {} data breaches!", breach_count);
    } else {
        println!("Password has not been seen in known data breaches.");
    }

    Ok(())
}
```
