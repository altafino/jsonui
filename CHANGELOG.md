# Changelog

All notable changes to ConfigDeck are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **Revert no longer offered for edits that a later change depends on through a
  shifted list index.** Removing or inserting a whole list item shifts the indices
  of the items after it. The review list could show Revert on an earlier edit
  inside one of those items even though a later edit relied on it, and clicking
  Revert then failed with "The path does not exist in the document". Paths that
  meet at an index of the same list now count as overlapping when either one
  addresses a whole item, so Revert stays disabled ("Revert later changes under
  <path> first"). Edits deeper inside two different items stay independent. Reapplying a
  draft after an outside change was not affected.
