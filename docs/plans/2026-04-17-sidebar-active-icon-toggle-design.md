# Sidebar Active Icon Toggle Design

**Date:** 2026-04-17

## Goal

Make the left asset-bar icons behave like common IDE sidebars:

- clicking the currently open icon collapses the asset sidebar
- clicking the same icon again reopens that panel
- clicking a different icon switches panels without an intermediate collapse

## Behavior

The selected destination remains remembered while the sidebar is collapsed. This keeps the last
active panel obvious and lets the same icon reopen the matching content immediately.

## Implementation Notes

- keep `active_sidebar_destination` unchanged when collapsing from the active icon
- only collapse when the requested destination already matches the active one and the sidebar is
  currently visible
- when the sidebar collapses, also hide the asset search row and create popover state
- preserve the existing "select different icon => open that panel" behavior

## Verification

- unit coverage in `tests/sidebar_navigation_spec.rs`
- UI binding coverage in `tests/sidebar_navigation_smoke.rs`
