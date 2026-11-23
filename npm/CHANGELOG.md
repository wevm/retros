# uphill

## 0.0.2

### Patch Changes

- 1281f68: Fixed an opportunity that groups a list's failing checks, which showed `pass → pass` and now shows `fail → pass`.

## 0.0.1

### Patch Changes

- d0d9be0: Renamed Retros to Uphill. The binary and npm package are `uphill`, the configuration is `uphill.yaml` or `uphill.toml`, reports go to `.uphill/`, scripts write to `$UPHILL_METRICS`, notes live under `refs/notes/uphill`, and the action keeps its comment under an `uphill` marker.
