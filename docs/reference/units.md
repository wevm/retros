# Units

A value's unit sets its kind: a size, a time, a count, or a check. Uphill converts each value to a base unit when it reads the line, and one metric uses one kind.

## Kinds

| Kind  | Units                       | Base unit    | Example  |
| ----- | --------------------------- | ------------ | -------- |
| Size  | `B`, `kB`, `MB`, `GB`       | bytes        | `45.6MB` |
| Time  | `ns`, `µs`, `us`, `ms`, `s` | nanoseconds  | `1.82ms` |
| Count | no unit                     | the number   | `412`    |
| Check | `pass`, `fail`, `skip`      | pass or fail | `pass`   |

Units are case-sensitive, so `KB` and `Ms` are errors. A unit follows the number without a space.

```text
install-size 45.6MB
startup 640us
modules 412
has-title index.html pass
```

## Sizes

Size units are decimal, so 1 kB is 1,000 bytes and 1 MB is 1,000,000 bytes. A size cannot be negative.

```text
bundle-size 6900B
bundle-size 6.9kB
```

## Times

`µs` and `us` are the same unit. A time cannot be negative. When a script's first run writes a time, Uphill runs the script 5 times and scores the median of each timing.

```text
import-time 5.3ms
slugify-time 938ns
```

## Counts

A count has no unit. It may be negative or have a fraction, such as a coverage percentage or a change in a score.

```text
coverage src/parse.ts 95.5
score-delta -3
```

## Checks

A check is `pass` or `fail`. A list of checks scores the share of items that pass, so a check needs no target and cannot have one.

A check that does not apply, such as an audit of video captions on a page without video, is `skip`. It counts toward neither side of a list. A check whose every line skips has no score and stays out of its category.

```text
links index.html pass
links about.html fail note "/team returns 404"
video-caption skip
```

## Display

Reports show sizes and times in the largest unit that keeps the number at 1 or more, with at most one decimal place. Counts show at most two decimal places.

`report.json` keeps base units: bytes for sizes and nanoseconds for times. A check item is `true` or `false`, and a list of checks has the share that pass, from 0 to 1.

```text
■  68  page-size  p90 31 kB (pricing.html)  target 20 kB
■  54  slugify-time  938.5 ns  target 500 ns
```
