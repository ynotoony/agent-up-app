<!-- Input: SPEC-012。 -->
<!-- Output: 签名方案。 -->
<!-- Pos: 发布签名政策。 -->

# Signing

Unsigned AgentUp builds are **preview** builds. They must not be called a stable release.

Stable macOS release requires Developer ID Application signing and notarization.
Stable Windows release requires Authenticode signing.

This repository does not embed certificates. CI or a human release operator signs outside Git.
