# json-describe

A command line tool that reads in a JSON file and outputs a JSON-like
generalized version of it.  This might help you understand the structure
of a JSON file you are unfamiliar with.

## Installation instructions

First, ensure you have [Rust installed](https://rustup.rs/).  Then, from some
directory you can write to:

```
$ git clone https://github.com/heycam/json-describe
...
$ cd json-describe
$ cargo install
```

## Usage

```
$ json-describe --help
Read a JSON file and output a generalized description of its structure

USAGE:
    json-describe [FILE]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <FILE>    specifies the input JSON file
```

For example, if the input file `/tmp/a.txt` contains:

```json
{
  "items": [
    { "name": "Xavier", "age": 33, "kids": ["Andrew", "Barbara", "Charlie"] },
    { "name": "Yulia", "age": 27, "kids": ["Doris", "Eric"] },
    { "name": "Zoe", "kids": ["Fran"] }
  ],
  "data": [false, true, 123],
  "date": "2019-02-09",
  "temperature": 17
}
```

then the result of running is `json-describe /tmp/a.txt` is:

```
{
    "data": Array (len 3) [
        Boolean,
        Number (123)
    ],
    "date": String ("2019-02-09"),
    "items": Array (len 3) [
        {
            "age": optional Number (27, 33),
            "kids": Array (len 1..3) [
                String ("Andrew", "Barbara", "Charlie", "Doris", ...)
            ],
            "name": String ("Xavier", "Yulia", "Zoe")
        }
    ],
    "temperature": Number (17)
}
```
