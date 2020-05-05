# Logsearch

Ability to search for patterns in log files and aggregate them by date.

This is most useful for log files where the date of the event may not be repeated
on every line, for instance:

```
[date] first line
  second line
  third line
[date2] ...
```

This is common for instance in java, where logs contain also stacktraces which
will span multiple lines. In that case, if you grep for a pattern which doesn't
appear on the line with the date, you don't know from the grep output when did
the event occur.

logsearch will keep track of what is the current date at any point in the file,
and will merge dates which are close to one another. In addition, it allows to
search for multiple patterns at once.

example session:

```
$ logsearch app.log "none available" "8.5.32 \(7.1.0\)" unique_active_station_machine_id
2020-04-23 07:32:52 -> 2020-04-23 07:32:52: [unique_active_station_machine_id] 4 matches
2020-04-23 07:42:46 -> 2020-04-23 07:42:46: [unique_active_station_machine_id] 4 matches
2020-04-23 11:43:37 -> 2020-04-23 11:43:37: [unique_active_station_machine_id] 4 matches
2020-04-23 16:04:55 -> 2020-04-23 17:17:09: [none available] 8916 matches
2020-04-23 17:17:28 -> 2020-04-23 17:17:31: [8.5.32 \(7.1.0\)] 2 matches
2020-04-23 18:19:47 -> 2020-04-23 20:30:30: [none available] 21901 matches
2020-04-23 20:30:54 -> 2020-04-23 20:30:55: [8.5.32 \(7.1.0\)] 2 matches
```

Here I search for three different patterns, and logsearch tracked them in the log file, and grouped
them by date intervals. On my machine, logsearch takes less than two seconds to produce this output
on a log file large 570Mb.

you can also patch the log file to search from through a pipe (which will allow to consume multiple
log files at once, too):

```
cat *.log | logsearch pattern
```

And this operation will also work if the log file is still being updated:

```
tail -f app.log | logsearch pattern
```

logsearch will also attempt to guess the date format used in the log file, but if that fails, you
can give the date format manually through the `--dtfmt` flag. The format must be given in the
[chrono date format spec](https://docs.rs/chrono/0.4.11/chrono/format/strftime). Pull requests
for common enough formats are welcome.

There is also another option `--mergesecs` to specify what is the maximum
interval between two events so that they get merged. The default is 5 minutes
(300 seconds).

logsearch is downloadable as a statically built linux x86-64 binary in the github releases page.
