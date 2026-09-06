#!/usr/bin/env perl
# Fail when documentation names a repository path that no longer exists.
#
# Scans Markdown files for inline code spans that look like repository paths
# (`crates/arto/src/config.rs`, `platform/macos/quicklook/`, ...) and checks
# that each one exists relative to the repository root. Paths that only rot
# silently after a move are the whole reason this exists; a link checker
# does not see them because they are code spans, not links.
#
# Skipped on purpose: fenced code blocks (examples and tree drawings), spans
# with glob or placeholder characters (`crates/*/assets`, `crates/<name>/`),
# and spans that carry a trailing `:line` reference (checked without it).
#
# Usage: check-doc-paths.pl [FILE...]   (defaults to the tracked *.md files)
use strict;
use warnings;
use utf8;
use open qw(:std :utf8);
use File::Basename qw(dirname);
use Cwd qw(abs_path);

my $root = abs_path(dirname(__FILE__) . '/../..');
chdir $root or die "cannot chdir to $root: $!";

my @files = @ARGV;
if (!@files) {
    @files = grep { length } split /\n/, qx(git ls-files '*.md' ':!frontend/node_modules');
}

# Top-level directories whose paths documentation is expected to name.
my $roots = qr{(?:crates|frontend|platform|nix|docs|samples|\.github|\.claude)};

# Build output that a fresh checkout does not have.
my $generated = qr{(?:^|/)(?:assets/frontend|frontend/dist|public/icons|target)(?:/|$)};

my $failures = 0;
for my $file (@files) {
    open my $fh, '<', $file or die "$file: $!";
    # The open fence, if any: its marker character and length. A fence
    # closes only on the same character with at least the same length, so a
    # ``` inside a ```` block does not end it. Fences may sit inside block
    # quotes (`> ```sh`).
    my ($fence_char, $fence_len);
    while (my $line = <$fh>) {
        if ($line =~ /^(?:\s*>)*\s*((`{3,})|(~{3,}))/) {
            my $marker = $1;
            my ($char, $len) = (substr($marker, 0, 1), length $marker);
            if (!defined $fence_char) {
                ($fence_char, $fence_len) = ($char, $len);
            }
            elsif ($char eq $fence_char && $len >= $fence_len) {
                undef $fence_char;
            }
            next;
        }
        next if defined $fence_char;
        while ($line =~ /`([^`\n]+)`/g) {
            my $span = $1;
            next unless $span =~ m{^\.?/?$roots/}; # only repository paths
            next if $span =~ /[*<>{}\[\]\s|]/;      # globs, placeholders, prose
            next if $span =~ /\.\.\./;              # elided paths
            next if $span =~ $generated;
            (my $path = $span) =~ s{^\./}{};
            $path =~ s/:\d+(?:-\d+)?$//;            # `file.rs:12` or `:12-20`
            $path =~ s{/$}{};
            next if -e $path;
            printf "%s:%d: `%s` does not exist\n", $file, $., $span;
            $failures++;
        }
    }
    close $fh;
}

if ($failures) {
    print STDERR "$failures stale path reference(s)\n";
    exit 1;
}
print "documentation paths ok\n";
