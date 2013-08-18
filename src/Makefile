#
# Makefile for bdd distribution
#

DOCS=../docs
MAIN=..
SRC=.
TMPDIR=/tmp/bdd-dist
DIR=bdd-0.2
TMP=$(TMPDIR)/$(DIR)

all:	dist

#
# Convert documentation to a more convinient end user format
#

docs:	$(DOCS)/bdd.1.pdf $(DOCS)/bdd.1.html $(DOCS)/Presentation.pdf $(SRC)/bdd.html

$(DOCS)/bdd.1.pdf:	$(MAIN)/bdd.1 /usr/bin/groff /usr/bin/gropdf
	/usr/bin/groff -man -T pdf -P -l $< > $@

$(DOCS)/bdd.1.html:	$(MAIN)/bdd.1 /usr/bin/groff /usr/bin/post-grohtml
	/usr/bin/groff -man -T html -P -l $< > $@

# openoffice must be running to get following to work!
$(DOCS)/Presentation.pdf:	$(SRC)/Presentation.odp /usr/bin/libreoffice
	/usr/bin/libreoffice --headless --convert-to pdf $<
	mv Presentation.pdf $(DOCS)/

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

/usr/bin/groff:
	echo $@ missing
	exit

/usr/bin/gropdf:
	echo $@ missing
	exit

/usr/bin/post-grohtml:
	echo $@ missing
	exit

/usr/bin/zip:
	echo $@ missing
	exit

/usr/bin/pydoc:
	echo $@ missing
	exit

/usr/bin/libreoffice:
	echo $@ missing
	exit

clean:
	cd $(MAIN); rm -f *~ bddc
	cd $(SRC); rm -f *~ bdd.html
	cd $(DOCS); rm -f *~ bdd.1.* Presentation.pdf
	cd $(MAIN)/test; rm -f *~

