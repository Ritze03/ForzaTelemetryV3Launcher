# Changelog Maintenance

**Mandatory working rule for agents, linked (force-loaded) from the repo-root `CLAUDE.md`.**

`CHANGELOG.md` in the **repo root** is the user-facing "what changed". Write for users with
no technical knowledge, not for developers.

## The shape

```
# Changelog

<short preamble naming the categories>

## YYYY-MM-DD

### Added
- **Short name**: one short sentence of what got better for the user.
```

- **`## YYYY-MM-DD`**: one block per calendar day, newest on top. Date-based policy, no
  version numbers (see [`documentation-version-policy.md`](documentation-version-policy.md)).
- **`### Added` / `### Fixed` / `### Removed` / `### Info`**: the only four categories.
  *Added* = new features, *Fixed* = bug and behaviour fixes, *Removed* = things taken
  out, *Info* = notes worth knowing. Omit a category that has no entries.
- **Bullets**: `- **Name**: sentence.` Hard-wrap at about 90 columns, continuation lines
  indented two spaces.

## Rules

- **Maintain it as you work.** Every commit that changes what users get adds a bullet in
  the **same commit**, under **today's** `## YYYY-MM-DD` block. Create that block at the
  top if it is missing; if it exists, add to it.
- **Never rewrite past blocks** or change their dates.
- **Plain language, what and never how.** Say what got better for the user. No file
  names, function names, flags, libraries, protocols, config keys, commit ids or
  implementation details. One short sentence per bullet; drop anything a non-technical
  user would not notice. Example: a fix for a variable-refresh-rate bug becomes
  `- **Variable refresh rate**: fixed an issue with variable refresh rate.`
- **Skip internal-only changes**: docs, refactors, tests, CI. A day with only such work
  adds no block.
- **Anonymize**: no hostnames, usernames, emails, real names, absolute home paths or
  private project names.
