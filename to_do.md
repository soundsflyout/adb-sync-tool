# To do

## Features to be added

- Allow a `delete files` to delete files in target directory that does not
belong in source directory.
- Properly handle admin permissions (current plan is to ignore
files/directories where the user does not have permissions).

## Known issues

## Possible considerations that I won't address

- If a directory has a trailing whitespace, such as 'hello /', the program will
  exit. This is because android will auto remove trailing whitespaces when making
  directories: i.e., android will make `hello/` when asked to make `hello /`
- If a directory does not have permissions to be written to, the program will
exit.
