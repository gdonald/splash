# splash
A log colorizer built in Rust

    Usage: splash [OPTIONS]

    Options:
      -m, --mode <MODE>  Log Parsing Mode (clf, ad-hoc)
      -p, --path <PATH>  Path to the log file
      -h, --help         Print help information
      -V, --version      Print version information

## Modes

Two modes are currently supported:

### Common Log Format

[https://en.wikipedia.org/wiki/Common_Log_Format](https://en.wikipedia.org/wiki/Common_Log_Format)

Nothing will be shown if the log file is not actually formatted in CLF format.  Use ad-hoc mode if you are unsure.

### Ad-hoc

Everything else.

