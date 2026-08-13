# To do

## Features to be added

- Allow user to quickly determine location of correct directory.
- Add a push feature as well as a pull feature.
- Allow for pushing a specific alias via json in the following format:

```
{
    "alias1": {
        "local_dir": foo1,
        "remote_dir": bar1,
        "allow_hidden": bool1
    },
    "alias2": {
        "local_dir": foo1,
        "remote_dir": bar1,
        "allow_hidden": bool1
    }
}
```

and then allow calling by `program push/pull alias1/alias2`.

Alternatively, allow pushing everything by `push/pull all`

## Known issues

## Possible considerations that I won't address

- If a directory has a trailing whitespace, such as 'hello /', the program will
  exit. This is because android will auto remove trailing whitespaces when making
  directories: i.e., android will make `hello/` when asked to make `hello /`
- If a directory does not have permissions to be written to, the program will
exit.
