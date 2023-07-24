# debctl

debctl is a CLI tool for managing apt repositories. It's intended as a
replacement for
[`add-apt-repository`](https://manpages.debian.org/buster/software-properties-common/add-apt-repository.1.en.html)
and [`apt-key`](https://manpages.debian.org/testing/apt/apt-key.8.en.html) that
supports the newer
[deb822](https://manpages.debian.org/stretch/apt/sources.list.5.en.html#DEB822-STYLE_FORMAT)
format and implements modern best practices for managing signing keys.

*What's wrong with the existing tools?*

- `add-apt-repository` adds repository entries to `/etc/apt/sources.list`, which
  is being phased out by the newer deb822 format.
- `apt-key` is deprecated because it trusts signing keys for *all* apt
  repositories instead of just the ones they're meant to be signing.

## Features

- Add new repositories to your system in deb822 format.
- Fetches signing keys from a local file path, a URL, or a keyserver and trusts
  them for only the repository they're signing via the `Signed-By` option.
- Migrate existing files from the old `.list` format to the deb822 `.sources`
  format.
- Keys can be installed in a keyring or inlined into the `.sources` file.
- Append new entries to existing `.sources` files.
- Generally encourages best practices, but provides escape hatches for doing
  weird stuff.

## Installation

Install [Rust](https://www.rust-lang.org/tools/install) and run:

```shell
cargo install debctl
```

This tool shells out to GnuPG for working with PGP keys, so you must have `gpg`
installed and available on your `PATH`.

## Examples

Let's add the [Docker apt
repository](https://docs.docker.com/engine/install/ubuntu/) to your system:

```shell
debctl new \
    --uri https://download.docker.com/linux/ubuntu \
    --key https://download.docker.com/linux/ubuntu/gpg \
    --component stable \
    docker
```

This downloads the signing key for the repository, installs it under
`/etc/apt/keyrings/`, and creates the repository entry at
`/etc/apt/sources.list.d/docker.sources`.

Here's what that file looks like:

```
Enabled: yes
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: jammy
Components: stable
Signed-By: /etc/apt/keyrings/docker-archive-keyring.gpg
```

Most documentation for third-party apt repositories directs users to use
`add-apt-repository`. This tool accepts the old-style syntax used by
`add-apt-repository` and converts it to deb822 syntax:

```shell
debctl add \
    --name docker \
    --key https://download.docker.com/linux/ubuntu/gpg \
    'deb [arch=amd64 lang=en_US] https://download.docker.com/linux/ubuntu jammy stable'
```

Here's the file that command generates:

```
Enabled: yes
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: jammy
Components: stable
Signed-By: /etc/apt/keyrings/docker-archive-keyring.gpg
Architectures: amd64
Languages: en_US
```

You can also convert existing old-style `.list` files to deb822-style `.sources`
files:

```shell
debctl convert --name docker
```

This replaces `/etc/apt/sources.list.d/docker.list` with
`/etc/apt/sources.list.d/docker.sources`.
