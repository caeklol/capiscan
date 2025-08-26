# capiscan

single binary minecraft server scanner \
not fast as others, but usable enough for me \
most importantly, a great learning experience \
name is a reference to [rapiscan](https://en.wikipedia.org/wiki/Rapiscan_Systems) but with a c.. caek.. idk

### usage
`./capiscan` should tell you how to do most of the things you need. but if you like reading:

##### `--asynchronous`
disabled by default, async mode sends packets without using a seperate outgoing port for \
each connection, the same way masscan does. `-a` for shorthand

##### `--source-port`
specifies the source port used for all connections. requires `--asynchronous`

##### `--target-file & --target`
use a set list of targets in masscan format (CIDR, dot-seperated IP range & single ip). \
`target` is parsed the same as each line in `target-file`

##### `--exclude-file`
exclude list, same format as above. if you do not provide one, the masscan list will be \
generated at `./exclude.conf`

##### `--state-file`
binary format state-file. not meant to be human read, this is just to resume in-progress \
scans. this is atomically saved unless you use `--no-atomic`. `-s` for shorthand

##### `--no-atomic`
<a name="my-custom-anchor-point"></a>
disables atomic save for binary format state file

### motivation
i really liked the [copyparty](https://github.com/9001/copyparty) philosophy, aka \
"reverse linux philosophy". do all (in this case most of) the things, and do an okay job. \
really tempers the urge of perfecting everything while still keeping myself satisfied. \
because well, thing never really aimed to do that in the first place
