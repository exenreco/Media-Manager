# Windows publisher signing

The current EXEs remain unsigned until a trusted signing identity is configured.
The release workflow supports Azure Artifact Signing with GitHub OIDC. No private
signing key is stored in GitHub, and no self-signed publisher certificate is used.

## One-time account setup

1. Create an Azure Artifact Signing account, complete identity validation and
   create a **Public Trust** certificate profile. Check Microsoft's current
   regional and applicant eligibility before purchasing the service.
2. Create a Microsoft Entra application/service principal and grant it the
   **Artifact Signing Certificate Profile Signer** role for that profile.
3. Create a GitHub Actions environment named `windows-release`. Restrict it to
   release tags and the trusted branch used for manual release runs. Add required
   reviewers if your release process needs them.
4. Configure an Entra federated credential for GitHub's issuer
   `https://token.actions.githubusercontent.com`, audience
   `api://AzureADTokenExchange`, and subject
   `repo:exenreco/Media-Manager:environment:windows-release`.
5. Add these environment secrets: `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`,
   `AZURE_SUBSCRIPTION_ID`. They identify the OIDC application; do not create a
   long-lived client secret for this workflow.
6. Add environment variables `WINDOWS_SIGNING_ENDPOINT` (your signing region's
   HTTPS endpoint), `WINDOWS_SIGNING_ACCOUNT` and `WINDOWS_SIGNING_PROFILE`.
7. Set the **repository** variable `WINDOWS_SIGNING_ENABLED` to `true` only after
   the profile and OIDC access are ready.

Run Platform releases manually with an existing version tag and platform
`windows`. Both the GUI and optional server are signed with SHA-256 and an
RFC 3161 timestamp. Packaging verifies Authenticode trust and timestamp presence
before creating the ZIP and its checksum. If signing is enabled but configuration,
signing or verification fails, publishing stops; it does not fall back to unsigned.

When signing is disabled, preview builds remain explicitly unsigned. Each new
Windows ZIP contains `WINDOWS-SIGNING.txt` describing its signing status. Existing
release ZIPs are unchanged until rebuilt. Signing identifies the publisher;
Windows reputation and organizational application policies still apply.

## References

- [Microsoft signing prerequisites and integrations](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations)
- [Official GitHub signing action](https://github.com/Azure/artifact-signing-action)
- [OIDC setup](https://github.com/Azure/artifact-signing-action/blob/main/docs/OIDC.md)

If you later obtain a hardware-backed certificate from another CA, its provider's
HSM integration will need a separate signing step; do not export private keys or
paste passwords into source files, issue reports or chat.
