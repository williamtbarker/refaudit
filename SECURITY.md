# Security policy

## Supported version

Security fixes currently target the latest `0.1.x` release.

## Reporting

Please report a suspected vulnerability privately through GitHub's security-advisory feature rather than a public issue. Include reproduction steps and impact, but do not include sensitive biological or clinical data.

## Data handling

`refaudit` is an offline command-line tool. It does not make network requests, execute input content, or collect telemetry. It reads the selected local input and writes only to standard output or the explicit `--output` path. Reports repeat sample and feature identifiers, so treat them with the same confidentiality as the input.
