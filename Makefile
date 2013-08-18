#
# Makefile to create bdd documentation and distributions
#

# creates PDF docs, pydoc and .tar.bz2 and .zip (to /tmp/bdd-dist)
docs_and_dists:
	cd src; make

# creates Debian package and source packages (to ..)
deb:
	dpkg-buildpackage -us -uc

clean:
	cd src; make clean
	rm -f bddc # byte compiled bdd
	rm -f debian/bdd.debhelper.log
	rm -f debian/bdd.substvars
	rm -rf debian/bdd/
	rm -f debian/files
	rm -f debian/*~
	rm -f debian/*#
