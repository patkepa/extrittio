# Security Policy

## Supported Versions

Only the current mainline branch is supported until the project adopts formal release branches.

## Reporting a Vulnerability

Do not open public issues for vulnerabilities. Use GitHub's private vulnerability reporting feature from the repository's **Security** tab. If that feature is unavailable, contact the repository owner privately before sharing details.

Include:

- Affected area and build configuration
- Steps to reproduce
- Expected and actual behavior
- Logs, screenshots, or proof-of-concept details when safe to share

## Secrets and Credentials

- Do not commit `.env` files, certificates, provisioning profiles, private keys, tokens, or passwords.
- Use Keychain for runtime secrets.
- Store runtime tokens in Keychain and never retain login passwords.
- Release and Staging builds require HTTPS.

## Transport Security

Debug can target a local HTTP backend. Staging and Release reject HTTP server URLs. Any App Transport Security exception should be narrow, documented, and tied to a specific development environment.
