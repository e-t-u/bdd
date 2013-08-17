# test script for bdd
# you should use this like: bash test/test.sh 2>&1 | more
# bdd must be ./bdd and it must be executable (chmod +x bdd)
# (some other shells will destroy quotation (e.g. dash))
# bdd must be excutable and located in ./bdd

echo "should print 1 (input file -)"
echo -en "1" | ./bdd --input-file=- --input-tuples --output-tuples
echo "should print 5 (input file /tmp/foo)"
echo -en "5" > /tmp/foo; ./bdd --input-file=/tmp/foo --input-tuples --output-tuples
echo "should give error message (nonexisting input file)"
rm -f /tmp/foo; ./bdd --input-file=/tmp/foo --input-tuples --output-tuples
echo "should print 2 (output file /tmp/foo)"
echo -en "2" | ./bdd --input-tuples --output-file=/tmp/foo --output-tuples; cat /tmp/foo
echo "should print 3 (output file -)"
echo -en "3" | ./bdd --input-tuples --output-file=- --output-tuples
echo "should print nothing (empty input file)"
./bdd --input-file=/dev/null | od -t x1
echo "should print: 06 (default input unit 8)"
echo -en "\006" | ./bdd  | od -t x1
echo "should print: (one per line) 0 0 0 0 0 1 1 0 (input bit at time)"
echo -en "\006" | ./bdd --input-unit=1 --output-integers
echo "should print: (one per line) 0 0 1 2 (input two bits at time)"
echo -en "\006" | ./bdd --input-unit=2 --output-integers
echo "should print: (one per line) 6 5 (input byte at time, several bytes)"
echo -en "\006\005" | ./bdd --input-unit=8 --output-integers
echo "should print: 12 (6<<1) (eof overflow one bit)"
echo -en "\006" | ./bdd --input-unit=9 --output-integers
echo "should print: 06000 (06<<12) (eof overflow more than one byte)"
echo -en "\006" | ./bdd --input-unit=20 --output-unit=20 --output-hex
echo "should print: 60 (skip bits, unit over byte border)"
echo -en "\006\005" | ./bdd --input-unit=8 --input-skip-bits=4 --count=1 --output-hex
echo "should print: 00 02 (gap, skip bits, unit over byte border, one bit overflow)"
echo -en "\006\005" | ./bdd --input-skip-bits=7 --input-unit=2 --input-gap=6 --output-hex
echo "should print: 03 (gap goes several bytes over eof)"
echo -en "\0300" | ./bdd --input-unit=2 --input-gap=17 --output-hex
echo "should print: 04 (gap ends to byte limit)"
echo -en "\040" | ./bdd --input-unit=5 --input-gap=3 --output-hex
echo "should print: 004 005 (pregap)"
echo -en "\0\010\020\030\040\050" | ./bdd --input-skip-bits=32 --input-pregap=2 --input-unit=3 --input-gap=3 --output-unit=8 | od -t o1
echo "should print 00 00 02 (no assert aligned)"
echo -en "\001" | ./bdd --input-unit=3 --output-hex
echo "should not warn, print 00 01 (assert aligned)"
echo -en "\001" | ./bdd --input-unit=4 --input-assert-aligned --output-hex
echo "should not warn, print (one per line) 1 2 (assert aligned followed by a gap)"
echo -en "\001\002" | ./bdd --input-skip-bits=4 --input-unit=4 --input-gap=4 --input-assert-aligned --output-integers
echo "should warn, print (one per line) 0,0,2 (assert aligned)"
echo -en "\001" | ./bdd --input-unit=3 --input-assert-aligned --output-tuples
echo "should warn (input use seek, stdin)"
echo -en "\006\005" | ./bdd --input-skip-bits=15 --input-use-seek --output-hex
echo "should print: 05 (input use seek with seekable input file)"
echo -en "\006\005" > /tmp/foo; ./bdd --input-file=/tmp/foo --input-skip-bits=13 --input-unit=3 --input-use-seek --output-hex
echo "should print 80\n03(input-reverse-bytes)"
echo -en "\001\0300" | ./bdd --input-reverse-bytes --input-unit=8 --output-hex
echo "should print 80\n03(input-reverse-unit)"
echo -en "\001\0300" | ./bdd --input-unit=8 --input-reverse-unit --output-hex
echo "should print c001 (combined input-reverse-bytes and unit)"
echo -en "\001\0300" | ./bdd --input-reverse-bytes --input-unit=16 --input-reverse-unit --output-pattern=16U --output-hex
echo "should print c001 (combined input-little-endian, same as previous)"
echo -en "\001\0300" | ./bdd --input-little-endian --input-unit=16 --output-pattern=16U --output-hex
echo "should print: 00 (input-zeros, unit over byte border, unit over a byte)"
./bdd --input-zeros --input-unit=16 --input-skip-bits=4 --count=1 --output-hex
echo "should give error message (input-zeros with --input-file)"
./bdd --input-zeros --input-file=/tmp/foo --count=1 --output-hex
echo "should give error message (input-ones with --input-file)"
./bdd --input-ones --input-file=/tmp/foo --count=1 --output-hex
echo "should give error message (input-random with --input-file)"
./bdd --input-random --input-file=/tmp/foo --count=1 --output-hex
echo "should print: 7fff (input-ones, unit over byte border, unit over a byte)"
./bdd --input-ones --input-unit=15 --input-skip-bits=4 --count=1 --output-unit=16 --output-hex
echo "should print: (one per line) 3 0 1 (input-counter, with skip and count)"
./bdd --input-counter --input-unit=2 --skip=3 --count=3 --output-integers
echo "should print two random-looking numbers 0-32767 (input-random, unit over a byte)"
./bdd --input-random --input-unit=15 --count=2 --output-integer
echo "should print 3 (input-integers with skip and count)"
echo -en "1\n2\n3\n4\n" | ./bdd --input-integers --skip=2 --count=1 --output-integers
echo "should give an error and print 1 (negative skip)"
echo -en "1\n2\n3\n4\n" | ./bdd --input-integers --skip=-1 --count=1 --output-integers
echo "should give an error and print 4 (negative count)"
echo -en "1\n2\n3\n4\n" | ./bdd --input-integers --skip=3 --count=-1 --output-integers
echo "should give two warnings and print (one per line) 0,3 (input-integers with letter and negative integer)"
echo -en "a\n-3\n" | ./bdd --input-integers --output-integers
echo "should print: 1,2\n3,4 (input-tuples)"
echo -en "1,2\n3,4" | ./bdd --input-tuples --output-tuples
echo "should print: 3,4 (input-tuples with skip and count)"
echo -en "1,2\n3,4\n5,6" | ./bdd --input-tuples --skip=1 --count=1 --output-tuples
echo "should print: 101 (input unit without pattern translated to tuple and usable in output-pattern)"
echo -en "\000" | ./bdd --count=1 --output-pattern=1o1U1o --output-bits
echo "should print: 45 (input pattern u)"
echo -en "\055" | ./bdd --input-pattern=8U --output-tuples
echo "should warn (both input unit and input pattern)"
echo -en "\055" | ./bdd --input-unit=8 --input-pattern=8U --output-tuples
echo "should print: 0,5,5,0,5,5  (input pattern u, several patterns, input unit not 8)"
echo -en "\055\055" | ./bdd --input-pattern=2U3U3U2U3U3U --output-tuples
echo "should print: 0,5,20,5,5 (input pattern u, pattern over byte border)"
echo -en "\055\055" | ./bdd --input-pattern=2U3U5U3U3U --output-tuples
echo "should print: 5,5,5,5 (input pattern x)"
echo -en "\055\055" | ./bdd --input-pattern=2x3U3U2x3U3U --output-tuples
echo "should give error (missing number in input pattern)"
echo "a" | ./bdd --input-pattern=x > /dev/null
echo "should give error (comma in input pattern)"
echo "a" | ./bdd --input-pattern=1x,3y > /dev/null
echo "should give error (size 0 in input pattern)"
echo "a" | ./bdd --input-pattern=0x > /dev/null
echo "should give error (wrong size for D in input pattern)"
echo "a" | ./bdd --input-pattern=12D > /dev/null
echo "should give error (only number as input pattern)"
echo "a" | ./bdd --input-pattern=123 > /dev/null
echo "should give an error (y as type character in input pattern)"
echo -en "\055" | ./bdd --input-pattern=2x2U2y2x > /dev/null
echo "should give an error (0 as bit length in input pattern)"
echo -en "\055" | ./bdd --input-pattern=6x2U0x > /dev/null
echo "should give an error (empty input tuple)"
echo -en "\055" | ./bdd --input-pattern=8x > /dev/null
echo "should print: 1,-1 (input pattern S)"
echo -en "\001\0377" | ./bdd --input-pattern=8S8S --output-tuples
echo "should print: 0,-1 (degenerate case of one bit S)"
echo -en "\001" | ./bdd --input-pattern=6x1S1S --output-tuples
echo "should print -128 (8S generates -maxint that takes 1 bit more than others)"
echo -en "\0200" | ./bdd --input-pattern=8S --output-tuples
echo "should print 1,128 (input pattern M, -maxint)"
echo -en "\0200" | ./bdd --input-pattern=8M --output-tuples
echo "should print: 1,1 (degenerate case of one bit M)"
echo -en "\001" | ./bdd --input-pattern=7x1M --output-tuples
echo "should print 12.3 (input pattern F)"
perl -e 'print pack("f", 12.3)' | ./bdd --input-pattern=32F --output-tuples
echo "should print 12.3 (input pattern D)"
perl -e 'print pack("d", 12.3)' | ./bdd --input-pattern=64D --output-tuples
echo "should not fail (multiple randoms converted to F)"
./bdd --input-random --count=1000 --input-pattern=32F --output-tuples > /dev/null
echo "should not fail (multiple randoms converted to D)"
./bdd --input-random --count=1000 --input-pattern=64D --output-tuples > /dev/null
echo "should print x,y,z (input pattern 8C)"
echo -en "xyz" | ./bdd --input-pattern=8C8C8C --output-tuples
echo "should print xyz (input pattern 24C)"
echo -en "xyz" | ./bdd --input-pattern=24C --output-tuples
echo "should print 128 (input pattern u)"
echo -en "\001" | ./bdd --input-pattern=8u --output-tuples
echo "should print aa 80 55 (input pattern u with other patterns)"
echo -en "\0125\001\0252" | ./bdd --input-pattern=8U8u8U --output-pattern=8u8U8u --output-hex
echo "should print -128 (input pattern s)"
echo -en "\001" | ./bdd --input-pattern=8s --output-tuples
echo "should print 12.3 (input pattern f)"
perl -e 'print pack("f", 12.3)' | ./bdd --input-pattern=32u --output-pattern=32U | ./bdd --input-pattern=32f --output-tuples
echo "should print 12.3 (input pattern d)"
perl -e 'print pack("d", 12.3)' | ./bdd --input-pattern=64u --output-pattern=64U | ./bdd --input-pattern=64d --output-tuples
echo "should print xyz (input pattern 24c)"
echo -en "xyz" | ./bdd --input-pattern=24u --output-pattern=24U | ./bdd --input-pattern=24c --output-tuples
echo "should print a,,,2,-1,5.5 (non-integer elements in --input-tuples, whitespace)"
echo -en "a,\",\",2, -1 , 5.5" | ./bdd --input-tuples --output-tuples
# TODO: quoting numbers with --input-tuples still does not work
echo "(TODO) should print 35, not 5 (quoted input-tuples interpreted as number)"
echo -en "\"5\"" | ./bdd --input-tuples --output-hex
echo "should print: 00 00 (count with infinite file)"
./bdd --input-file=/dev/zero --count=2 --output-hex
echo "should print: 32\n33 (skip, count)"
echo -en "12345" | ./bdd --input-unit=8 --skip=1 --count=2 --output-hex
echo "should print nothing (skip past eof)"
echo -en "12345" | ./bdd --input-unit=8 --skip=200 --output-tuples
echo "should print 5 (output pattern U)"
echo -en "\005" | ./bdd --input-unit=8 --output-pattern=8U --output-integers
echo "should print 00 00 05 00 00 05 (several output pattern U)"
echo -en "\005\005" | ./bdd --input-pattern=2U3U3U2U3U3U --output-pattern=8U8U8U8U8U8U | od -t x1
echo "should print 14 50 (varying size output pattern U, eof fill with zero)"
echo -en "\005\005" | ./bdd --input-pattern=2U3U3U --output-pattern=1U2U3U | od -t x1
echo "should print 12 (several pattern, ignore extra tuples in pattern)"
echo -en "1,2,3" | ./bdd --input-tuples --output-pattern=4U4U | od -t x1
echo "should print 07 (output pattern S)"
echo -en "-1" | ./bdd --input-tuples --output-pattern=5z3S | od -t x1
echo "should print 0e (output pattern M)"
echo -en "1,2" | ./bdd --input-tuples --output-pattern=4z4M | od -t x1
echo "should print 01 02 (output pattern 8C)"
echo -en "1\n2" | ./bdd --input-tuples --output-pattern=8C | od -t x1
echo "should print 00 01 00 02 (output pattern n*8C)"
echo -en "0,1\n0,2" | ./bdd --input-tuples --output-pattern=8C8C | od -t x1
echo "should print 41 and warning (output pattern 7C)"
echo -en "A" | ./bdd --input-tuples --output-pattern=1z7C | od -t x1
echo "should print 0 (oversize integer is truncated with S)"
echo -en "-16" | ./bdd --input-tuples --output-pattern=4z4S | od -t x1
echo "should print 0 (oversize integer is truncated with M)"
echo -en "1,16" | ./bdd --input-tuples --output-pattern=4z4M | od -t x1
echo "should print ~123.45 (output pattern F)"
echo -en "123.45\n" | ./bdd --input-tuples --output-pattern=32F | perl -e 'print unpack("f",<>)' ; echo -en "\n"
echo "should print ~123.45 (output pattern D)"
echo -en "123.45\n" | ./bdd --input-tuples --output-pattern=64D | perl -e 'print unpack("d",<>)' ; echo -en "\n"
echo "should print 123 (output pattern u)"
echo -en "\0123" | ./bdd --input-pattern=8u --output-pattern=8u | od -t o1
echo "should print 123 (output pattern s)"
echo -en "\0123" | ./bdd --input-pattern=8s --output-pattern=8s | od -t o1
echo "should print 7F (output pattern m)(-2 = FE, reversed) "
echo -en "1,2" | ./bdd --input-tuples --output-pattern=8m | od -t x1
echo "should print ~123.45 (output pattern f)"
echo -en "123.45" | ./bdd --input-tuples --output-pattern=32f --output-reverse-unit | perl -e 'print unpack("f",<>)'; echo -en "\n"
echo "should print ~123.45 (output pattern d)"
echo -en "123.45" | ./bdd --input-tuples --output-pattern=64d --output-reverse-unit | perl -e 'print unpack("d",<>)'; echo -en "\n"
echo "should print 123 (output pattern c)"
echo -en "123\n" | ./bdd --input-pattern=32c --output-pattern=32c
echo "should print abcd (output pattern c)"
echo -en "abcd" | ./bdd --input-tuples --output-pattern=32c --output-reverse-unit ; echo -en "\n"
echo "should print 3 times 0f or 0e (output pattern z, o and r)"
./bdd --input-zeros --count=3 --output-pattern=4z3o1r --output-hex
# TODO: error messages mixing different internal types and output patterns
echo "should give error message (tuple 2 values, output pattern 3)"
./bdd --input-zeros --count=1 --input-pattern=3U3U --output-pattern=3U3U3U > /dev/null
echo "should print random 100-bit number (large r)"
./bdd --input-zeros --count=1 --output-pattern=100r --output-hex
echo "should print ff ff ff ff (output unit is taken from pattern)"
./bdd --input-ones --count=32 --output-pattern=1u | od -t x1 
echo "should print 99 99 99 99 (output unit is taken from pattern)"
./bdd --input-ones --input-pattern=1U1U --count=8 --output-pattern=1U2z1U | od -t x1
echo "should print 01 01 01 01 (output unit defaults to 8)"
./bdd --input-ones --input-unit=1 --count=4 | od -t x1
echo "should print 77 77 (output unit forced, no pattern, fill zeros)"
./bdd --input-ones --input-unit=3 --count=4 --output-unit=4 | od -t x1
echo "should print ff ff ff ff (output unit forced, no pattern, cut left, over byte)"
./bdd --input-ones --input-unit=19 --count=2 --output-unit=16 | od -t x1
echo "should print 01 02 03 04 (output reverse bytes)"
echo -en "\01\02\03\04" | ./bdd --input-unit=32 --input-reverse-bytes --output-unit=32 --output-reverse-bytes | od -t x1
echo "should print 04 03 02 01 (output reverse unit, over one byte unit)"
echo -en "1,2,3,4" | ./bdd --input-tuples --output-pattern=8u8u8u8u --output-reverse-unit | od -t x1
echo "should print 02 01 04 03 (output little endian)"
echo -en "\01\02\03\04" | ./bdd --input-unit=16 --output-unit=16 --output-little-endian | od -t x1
echo "should print 2,1\n4,3 (rearrange)"
echo -en "1,2\n3,4" | ./bdd --input-tuples --rearrange=1,0 --output-tuples
echo "should print 2,1\n4,3 (rearrange, negative indexes)"
echo -en "1,9,2\n3,9,4" | ./bdd --input-tuples --rearrange=-1,0 --output-tuples
echo "should give a warinng (rearrange, index out of range)"
echo -en "1,9,2" | ./bdd --input-tuples --rearrange=5 --output-tuples
echo "should print 1,2 (rearrange, missing fieldlist interpreted as no option at all)"
echo -en "1,2" | ./bdd --input-tuples --rearrange= --output-tuples
echo "should print 1,1 (rearrange, duplicating and dropping fields)"
echo -en "1,2" | ./bdd --input-tuples --rearrange=0,0 --output-tuples
echo "should give an error (rearrange, non-number field)"
echo -en "1,2" | ./bdd --input-tuples --rearrange=0,, --output-tuples
echo "should print -63,100 (cut maxint)"
echo -en "-100,100" | ./bdd --input-tuples --cut-maxint=0,63 --output-tuples
echo "should print -100,63 (cut maxint)"
echo -en "-100,100" | ./bdd --input-tuples --cut-maxint=1,63 --output-tuples
echo "should print 2 (remove right)"
echo -en "\0277" | ./bdd --input-unit=8 --remove-right=0,6 --output-tuples
echo "should print 10 (xor)"
echo -en "\005" | ./bdd --input-unit=8 --xor=0,4 --output-tuples
echo "should print 85 7f (input pattern M, cut maxint, turn sign)"
echo -en "\005\0200" | ./bdd --input-pattern="8M" --cut-maxint=1,127 --xor=0,1 --output-pattern="1U7U" | od -t x1
echo "should print 999 (abs)"
echo -en "-999" | ./bdd --input-tuples --abs=0,0 --output-tuples
echo "should print 1,0,9 (sign)"
echo -en "-9,0,9" | ./bdd --input-tuples --sign=0,0 --output-tuples
echo "should print -9,0,9 (sign)"
echo -en "-9,0,9" | ./bdd --input-tuples --sign=1,0 --output-tuples
echo "should print -9,0,0 (sign, negative field)"
echo -en "-9,0,9" | ./bdd --input-tuples --sign=-1,0 --output-tuples
echo "should print two lines of 1-digit 0/1 hex (output-hex, 1 bits)"
./bdd --input-counter --count=64 --output-unit=1 --output-hex
echo "should print two lines of 2-digit 0..1F hex (output-hex, 8 bits)"
./bdd --input-counter --count=32 --output-unit=8 --output-hex
echo "should print two lines of n-digit random hex (output-hex, 999 bits)"
./bdd --input-random --input-unit=999 --count=2 --output-unit=999 --output-hex
echo "should print two lines of 8-bit 0..1111 values (output-binary)"
./bdd --input-counter --count=16 --output-unit=8 --output-bits
echo "---testing all the shortcuts in the processing pipeline---"
echo "1 - bitstream -> manipulation (no input pattern)"
./bdd --input-zeros --count=1 --xor=0,1 --output-integers
echo "2 bitstream -> output-pattern (no input pattern)"
./bdd --input-counter --count=1 --skip=2 --output-pattern=8U --output-integers
echo "3 bitstream -> bitstream (no input pattern)"
./bdd --input-counter --count=1 --skip=3 --output-integers
echo "4 input-tuples -> manipulation"
echo -en "3" | ./bdd --input-tuples --xor=0,3 --output-integers
echo "5 input-pattern -> bitstream"
./bdd --input-counter --count=1 --skip=5 --input-pattern=3U --output-integers
echo "6 manipulation -> bitstream"
./bdd --input-ones --count=1 --input-unit=1 --xor=0,3 --output-integers
echo "7 manipulation -> output-tuples"
./bdd --input-zeros --count=1 --xor=0,3 --output-integers
echo "should print FF 05 FF 05 (merge file -)"
echo -en "\005\005" |./bdd --input-ones --count=2 --merge-file=- | od -t x1
echo "should print 00 06 (merge file /tmp/foo, extra units in merge file)"
echo -en "\006\007\008" > /tmp/foo; ./bdd --input-zeros --count=1 --merge-file=/tmp/foo | od -t x1
echo "should give error message (nonexisting merge file)"
rm -f /tmp/foo; ./bdd --merge-file=/tmp/foo
echo "should print 00 07 00 and error premature end of merge file"
echo -en "\007" |./bdd --input-zeros --count=2 --merge-file=- --output-hex
echo "should print 80 C0 and error (flushes last byte in case of premature end of merge file)"
echo -en "\001" |./bdd --input-ones --input-unit=1 --output-unit=1 --count=2 --merge-file=- | od -t x1
echo "should print 92 60 (unit size not 8 in merge file, <> normal output unit size)"
echo -en "\002" | ./bdd --input-ones --input-unit=1 --output-unit=1 --count=4 --merge-file=- --merge-unit=2 | od -t x1
echo "should print: 45 (--merge-pregap)"
echo -en "\0\010\020\030\040\050" | ./bdd --input-zeros --count=2 --output-unit=1 --merge-file=- --merge-skip-bits=32 --merge-pregap=2 --merge-unit=3 --merge-gap=3 | od -t x1
echo "should print 01 FF 02 (--merge-copy-first)"
echo -en "\01\02" | ./bdd --input-ones --count=1 --merge-file=- --merge-unit=8 --merge-copy-first=8 | od -t x1
echo "should print 01 02 FF 03 FF 04  (--merge-copy-first and --merge-skip-bits at the same time)"
./bdd --input-counter --count=10 | ./bdd --input-ones --count=2 --merge-file=- --merge-skip-bits=8 --merge-copy-first=16 | od -t x1

# TODO: merge assert aligned
# TODO: merge little-endian etc.
