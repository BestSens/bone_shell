# bone_shell
The bone_shell is a shell application for communication with the BeMoS API.

It has support for sending and receiving json encoded commands over a TCP connection either as msgpack or plain text. It also allows to  do a login and allows rudimentary displaying of raw and DirectView data.

It also has some convenienance functions like command completion in shell mode.

## Commandline parameters
Usage of the tool is like follows:

```
bone_shell [OPTION...] command
		--version			print version string
		--connect arg		connect to specified ip (default: localhost)
		--port arg			connect to specified port (default: 6451 / 6450)
	-m,	--msgpack			compress sent and received data with msgpack
	-n,	--no-pretty			don't do a pretty print of received JSON, just output it in one line
	-r,	--response-time		every command executed in a TTY environment shows displays the execution time of the command
		--username arg		supply a username to initiate a login before executing command
		--password arg		if a username is set, a password is mandatory
		--api arg			api version that is used on command completion (default: 2)
		--unencrypted		use unencrypted connection
```

## Pipe & Command-Mode
When a command is supplied either as an argument or via pipig the shell is executing this command and outputs the response either to stdout or piping it to the next executable.

Quotes and other special characters in the command need to be escaped depending on the used shell.

### Examples
```shell
$ bone_shell '{"command":"date"}'
```
```
{
	"command": "date",
	"payload": {
		"date": "2021-05-04 21:08:33",
		"timeserver": "0.de.pool.ntp.org 1.de.pool.ntp.org 2.de.pool.ntp.org 3.de.pool.ntp.org",
		"timesync": true,
		"timezone": "Europe/Berlin",
		"timezone offset": "+0200"
	}
}
```

```shell
$ echo '{"command":"date"}' | bone_shell
```
```
{
	"command": "date",
	"payload": {
		"date": "2021-05-04 21:08:33",
		"timeserver": "0.de.pool.ntp.org 1.de.pool.ntp.org 2.de.pool.ntp.org 3.de.pool.ntp.org",
		"timesync": true,
		"timezone": "Europe/Berlin",
		"timezone offset": "+0200"
	}
}
```

```shell
$ echo '{"command":"date"}' | bone_shell --no-pretty | jq
```
```json
{
	"command": "date",
	"payload": {
		"date": "2021-05-04 21:09:54",
		"timeserver": "0.de.pool.ntp.org 1.de.pool.ntp.org 2.de.pool.ntp.org 3.de.pool.ntp.org",
		"timesync": true,
		"timezone": "Europe/Berlin",
		"timezone offset": "+0200"
	}
}
```

## Shell mode
When the program is executed without a supplied command it enters shell mode. On this mode you get an interactive shell for sending multiple commands. You also benefit from auto command completion.

If a command starts with '{' or '[' it is determined as a JSON string and is sent to the BeMoS as supplied. Otherwise it does a completion like follows:

```shell
> date
# {"command":"date"}
```

You may also add a payload after a whitespace:

```shell
> channel_data {"name":"external_data"}
# {"command":"channel_data", "payload":{"name":"external_data"}}
```

### Login
If you have not supplied a username/password via commandline or want to change user, you may use the `login` shortcut to allow supplying username and password directly from stdin and doing the correct commands for you to get authenticated for this session.

## Raw data
Raw and DirectView data is displayed in cute ASCII graphs for a quick overview. For raw data you also get some statistical moments.