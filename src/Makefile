#
# Makefile for bdd distribution
#

DOCS=../docs
MAIN=..
SRC=.
TMPDIR=/tmp/bdd-dist
DIR=bdd-0,2
TMP=$(TMPDIR)/$(DIR)

all:	dist

#
# Convert documentation to a more convinient end user format
#

docs:	$(DOCS)/bdd.1.pdf $(DOCS)/bdd.1.html $(DOCS)/Presentation.pdf $(SRC)/bdd.html

$(DOCS)/bdd.1.pdf:	$(DOCS)/bdd.1.ps /usr/bin/ps2pdf
	ps2pdf $< $@

$(DOCS)/bdd.1.ps:	$(MAIN)/bdd.1
	groff -man $< > $@

$(DOCS)/bdd.1.html:	$(MAIN)/bdd.1
	groff -man -T html -P -l $< > $@

# openoffice must be running to get following to work!
$(DOCS)/Presentation.pdf:	$(SRC)/Presentation.odp /usr/bin/unoconv
	echo "Openoffice/Libreoffice must be running!"
	unoconv --stdout $< > $@

$(SRC)/bdd.html:	$(MAIN)/bdd /usr/bin/pydoc
	cd $(SRC); pydoc -w $<

#
# Packaging distribution
#

dist:	/tmp/bdd.tar.bz2 /tmp/bdd.zip

/tmp/bdd.tar.bz2:	$(TMP)
	cd $(TMPDIR); tar cvfj $@ $(DIR)

/tmp/bdd.zip:	$(TMP) /usr/bin/zip
	cd $(TMPDIR); zip -r $@ $(DIR)

#
# Copy of files in the work directory to the distribution
#

distfiles:  $(MAIN)/bdd $(MAIN)/bdd.1 docs $(SRC)/Makefile $(SRC)/Presentation.odp $(MAIN)/test/test.sh

$(TMP):	distfiles
    # main
	install -D $(MAIN)/bdd $@/bdd
	install -D $(MAIN)/bdd.1 $@/bdd.1
	# docs
	install -D $(DOCS)/Presentation.pdf $@/docs/Presentation.pdf
	install -D $(DOCS)/bdd.1.html $@/docs/bdd.1.html
		install -D $(DOCS)/bdd.1.pdf $@/docs/bdd.1.pdf
	install -D $(DOCS)/TODO $@/docs/TODO
	# src
	install -D $(SRC)/Makefile $@/src/Makefile
	install -D $(SRC)/Presentation.odp $@/src/Presentation.odp
	install -D $(SRC)/bdd.html $@/src/bdd.html
	# test
	install -D $(MAIN)/test/test.sh $@/test/test.sh

#
# Warnings if conversion tools are missing
#

/usr/bin/ps2pdf:
	echo $@ missing
	exit

/usr/bin/zip:
	echo $@ missing
	exit

/usr/bin/pydoc:
	echo $@ missing
	exit

/usr/bin/unoconv:
	echo $@ missing
	exit

clean:
	cd $(MAIN); rm -f *~
	cd $(SRC); rm -f *~ bdd.html
	cd $(DOCS); rm -f *~ bdd.1.* Presentation.pdf
	cd $(MAIN)/test; rm -f *~

