bdd
===

A command line tool to interpret, manipulate, merge and create bit
streams of any bit size with advanced bit field interpretation.

The program is one Python script, ./bdd.

Read bdd.1 and src/Presentation.odp to get more information how to use
the command.

More readable documentation is generated with Makefile in src
directory. It generates:

- bdd.1.html
- bdd.1.pdf
- Presentation.pdf
- bdd.html (pydoc file)

It also generates distribution files

- /tmp/bdd-dist/bdd.tar.bz2
- /tmp/bdd-dist/bdd.zip

Containing the source files and generated documentation.

To generate the documentation and distribution files, the following
tools are needed:

- libreoffice
- groff (with gropdf and post-grohtml)
- pydoc
- zip
- tar

/debian directory contains the configuration files needed to generate
debian package of the files. Debian archive generation is not yet
included in the Makefile and debian directory is not included in the
distribution archives.

Debian package is available in the previous location of this package:

- http://sourceforge.net/projects/bdd