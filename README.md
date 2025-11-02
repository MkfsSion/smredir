# smredir

Canokey Pigeon with 1.5.2 firmware has a known bug regarding USB reset, which prevents USB/IP usage.
This can be somewhat workarounded by set `usbcore.quirks` with `e` flag in Linux after v4.17-rc1.

On Windows host, however, their is no public way for user to configure such USB stack workaround. There are kernel shim engine and device comppatibility database (drvmain.sdb) but only Microsoft knows what flag should be applied.

This project workarounds this by creating a specialized USB/IP server to relay and emulate communication between host and guest.

## Supported interface
- FIDO/U2F[^1]
- WebUSB[^2]
- CCID[^3]

[^1]: Only `fido2-token -I ` tested.

[^2]: Only information read without admin PIN in OpenPGP page of Canokey Legacy Console tested.

[^3]: Only OpenPGP tested.

## Usage

Plug Canokey Pigeon in Windows host, then build and run the project with `cargo run` with Administrator privilege.

Administrator privilge is required for now for FIDO/U2F to work. You can replace this interface with reserved interface if you want to run it without Administrator privilege.

You may also want to change log level or path to protect sensitive data.

## Known issues
1. WebUSB is not reliable.

- This may caused by some interaction of CCID interface by host applications.
- This can be somewhat workaround by refreshing multiple times.
