# punch

__punch__ is a (very) minimal time-tracking application. Remember [punch clocks](https://en.wikipedia.org/wiki/Time_clock)? That's it.

## Usage

```
punch in

# do some work

punch out

punch card
Previously punched in between 2016-12-03 13:14:17 UTC and 2016-12-03 18:52:21 UTC (05h38m)

punch card -m
2016-12-01UTC: 00h26m
2016-12-02UTC: 00h38m
2016-12-03UTC: 05h38m

Total: 06h42m
```

## Options

`punch card` has two options:

   * `-w` week to date summary
   * `-m` month to date summary

## Installation

Ensure you have `rust` installed, then

```
cargo install punch
```

