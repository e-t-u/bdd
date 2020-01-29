#
# Input bit stream handlers
#
# All the input stream handlers return a unit generator that can be used as>
#   reader = InputBitStream()
#   for unit in reader.units():
#       ...
# or
#    unit = next(reader)
# or
#    [unit for unit in itertools.isplit(reader.units(), 100]
#
# The unit is simply Python unlimited integer. That is not very efficient but it
# allows us to handle literally any bitstream unit.
#
# The main reader splits a file to units bit by bit. But let's have a view to simpler
# unit readers first to understand unit level processing.
#
# ZeroStream()
# is a reader that returns only units containing a zero.
#
# OneStream()
# is a reader returns bits one the full unit size.
# if unit size is 8 bits, generator returns numbers 255
# (0xFF) as the unit. If unit is 2, it returns numbers 3.
# Default unit size is always 8 bits (byte).
#
# RandomStream()
# is a reader that returns random numbers as units. Units are
# of the size of the unit given to the reader. Random numbers
# are based on Linux urandom() that gives safe random numbers,
# not pseudorandom numbers.
#
# CounterStream()
# is a reader that returns increasing numbers. Units are
# numbered starting from zero and the number is returned as the
# unit. Number is truncated to the unit size, if unit size is
# 8 bits, after unit 255 comes unit 0 again.
#
# Counter stream is controlled by generic unit counter/stepper
# parameters. They are available in all the unit readers but
# they do not do much for zeros, ones or random streams. Only
# units parameter affects amount of units generated.
#
# With CounterStream certain numbers are skipped based on
# counter/stepper parameters:
# - skip - how many units are skipped in the beginning (default 0)
# - step - when we emit one unit, this many succeeding units
#          are ignored before the next (default 0)
# - units - how many units we return (default unlimited,
#          until end of input stream of when receiver stops receiving
#
# The main use of these parameters are extracting data from
# certain slot of the input and/or every n:th unit. With units
# parameter we also make generator limited and return only some units
#
# Counter/stepper parameters are given by using a Counter object.
#
# For example with CounterStream(Counter(skip=1, step=1, units=2))
# returns units 1 and 3, unit 0 is skipped and unit 2 stepped.
#
# IntegerInputStream()
# Integer input stream is an easy way to use only the output
# part of the bit handling. It allows input file that contains
# units as normal text integers.
#
# Input file is gives as a file descriptor. Therefore, also
# a string can be used as an input using io.StringIO strings.
# For example:
#     reader = IntegerInputStream(fd=io.StringIO("12\n34\n56"))
#
# Empty lines and comment lines starting with "#" are skipped.
#
# Normal fitting to unit size and counter/stepper applies to integer stream.
# If unit is:
# - non number - warning to stderr and unit is zero
# - negative integer - warning to stderr and changed to positive
# - too big to the unit - warning to stderr and MSB cut off
#
# Finally the real thing:
# FileInputStream()
#
# Reads a file extracting bit level units and similar unit-level
# handling for the units as simpler readers. It also contains
# several ways to handle endianness of the bit unit.
#
# Also file input stream allows using strings as files.
# Instead of StringIO, ByteIO is used. For example:
#     reader = FileInputStream(fd=io.BytesIO(b"\x01\02\03"))
#     [unit for unit in reader.units()] -> [1, 2, 3]
#
# Bit stream has a bit-level skipper-stepper that is quite
# similar as the unit level counter-stepper.
#



from os import urandom
from sys import stderr


class InputBitStream:
    """Inputbitstream splits input to unit size bits units.

    All inputbitstreams implement counter functionality: skip and count,
    but because zero, one and random do not need to care of skip
    (and zero not even unit size), they implement them internally.
    Integers and file inputstreams require parameter of type Counter class.

    InputBitStream is used like:
      counter = Counter(skip, count, step)
      ibs = InputBitStream(counter=counter)
      ibs.set_unit_size(8)
      ibs.do_skip()
      for unit in ibs.units():
          use(unit)

    """

    def __init__(self, counter=None, fd=None, unit_size=8):
        """By default, all input streams have counter and unit arguments"""
        if counter:
            assert isinstance(counter, Counter)
        # TODO: Does not work in Python3
        # if fd:
        #    assert isinstance(fd, file)
        if counter is None:
            self.counter = Counter()    # default is continuous infinite bit stream
        else:
            self.counter = counter
        self.fd = fd
        self.unit_size = unit_size

    def set_unit_size(self, bits):
        assert bits > 0
        """Unit size can be set after initialization using this method."""
        self.unit_size = bits

    def get_unit_size(self):
        """Get unit size of the input stream."""
        return self.unit_size

    def do_skip(self):
        """If subclass needs to do skip after the unit is set,
        it redefines do_skip() method to do that.
        """
        pass

    def units(self):
        """Generator that returns values in bit stream one unit at time"""
        assert False, "You can not use virtual class InputBitStream directly"

    def __iter__(self):
        return self


class ZeroStream(InputBitStream):
    """Input stream that returns always units of zeros"""

    def units(self):
        for action in self.counter:
            if action == "skip":
                continue
            elif action == "use":
                yield 0
            elif action == "step":
                continue
            elif action == "finished":
                return
            else:
                assert False, "Unknown answer from counter"


class OneStream(InputBitStream):
    """Input stream that returns always unit full of ones."""

    def units(self):
        for action in self.counter:
            if action == "skip":
                continue
            elif action == "use":
                yield (1 << self.unit_size) - 1
            elif action == "step":
                continue
            elif action == "finished":
                return
            else:
                assert False, "Unknown answer from counter"


class RandomStream(InputBitStream):
    """Input stream that returns always unit full of random bits"""

    def units(self):
        # We waste ramdomness and read full random bytes
        # for each unit.
        for action in self.counter:
            if action == "skip":
                continue
            elif action == "use":
                needed_bytes = self.unit_size // 8 + 1
                unitmask = (1 << self.unit_size) - 1
                random = 0
                for randombyte in range(needed_bytes):
                    random *= 256
                    random += ord(urandom(1))
                random &= unitmask
                yield random
            elif action == "step":
                continue
            elif action == "finished":
                return
            else:
                assert False, "Unknown answer from counter"


class CounterStream(InputBitStream):
    """Input stream that returns units that are increasing.
    If Counter is given, the numbering
    comes as if in the input stream there were units counting
    starting from 0 and skip and step jumps over these numbers.
    The counter wraps around according the unit size."""

    def units(self):
        unit_number = -1
        unit_mask = (1 << self.unit_size) - 1
        for action in self.counter:
            unit_number += 1
            unit_number &= unit_mask
            if action == "skip":
                continue
            elif action == "use":
                yield unit_number
            elif action == "step":
                continue
            elif action == "finished":
                return
            else:
                assert False, "Unknown answer from counter"


class IntegerInputStream(InputBitStream):
    """Input stream that returns units from integers read from input file.
    Argument fd must be an open file descriptor OR it can be a StringIO object
    that is read instead of the open file. Empty lines and lines starting
    with "#" are ignored. As usual, Counter can be used to take a sample of input."""

    def units(self):
        unit_mask = (1 << self.unit_size) - 1
        file = self.fd
        for action in self.counter:
            line = next(file)
            while line.strip().startswith('#') or line == '\n':
                line = next(file)    # must not ask for a new action
            line = line.strip()
            if action == "skip":
                continue
            elif action == "use":
                try:
                    i = int(line)
                except ValueError:
                    stderr.write(f"Non-integer {line} interpreted as zero\n")
                    i = 0
                if i < 0:
                    stderr.write(f"Negative integer {line} interpreted as positive\n")
                    i = abs(i)
                if i != (i & unit_mask):
                    stderr.write(f"Integer {line} larger than unit size, MSB truncated\n")
                    i &= unit_mask
                yield i
            elif action == "step":
                continue
            elif action == "finished":
                return
            else:
                assert False, "Unknown answer from counter"


class FileInputStream(InputBitStream):
    """Reads units from a file."""

    def __init__(self, fd, counter=None, skip_bits=0,
                 skip_units=0, gap=0,
                 assert_aligned=False, use_seek=False,
                 reverse_bytes=False, reverse_unit=False):
        assert skip_bits >= 0 and skip_units >= 0 and gap >= 0
        InputBitStream.__init__(self, counter=counter, fd=fd)
        self.skip_bits = skip_bits
        self.skip_units = skip_units
        self.gap = gap
        self.assert_aligned = assert_aligned
        self.use_seek = use_seek
        self.reverse_bytes = reverse_bytes
        self.reverse_unit = reverse_unit
        self.eof = False

    def units(self):
        for action in self.counter:
            unit = self._read_unit()
            if unit is "" or self.eof:
                return
            if action == "skip":
                continue
            elif action == "use":
                yield unit
            elif action == "step":
                continue
            elif action == "finished":
                return
            else:
                assert False, "Unknown answer from counter"

    def _read_unit(self):
        byte = self._read_byte()
        if byte is None:
            return None
        else:
            return ord(byte)

    def _read_byte(self):
        """Read a byte as an integer from data.

        Detects end of file and sets self.eof if found.
        Reverses byte read if self.reverse_bytes.

        Execution can not stop immediately to the eof,
        because the last unit must be output filled with zeros.
        Read is normally called only when there is need
        for a unit. This means that if eof occurs,
        there is no more than a unit to write. If _read
        is called for look-ahead, look-ahead is exactly one
        byte and it is read when buffer is empty. Then the
        whole byte is either usable data or it is eof/zero.

        """
        c = self.fd.read(1)
        if c is b"":
            self.eof = True
            return None
        # if self.reverse_bytes:
        #     val = _reverse(val, 8)
        return c

    def do_skip(self):
        """Do bit skip in input file.

        Assumes that self.unit is set to input unit size.
        Initializes buffer self.buffer with pointer
        self.bits_in_buffer.

        """
        self.skip_bits += self.unit_size * self.skip_units
        skip_bytes = self.skip_bits / 8
        if skip_bytes > 0:
            if self.use_seek == True:
                try:
                    self.file.seek(skip_bytes)
                except IOError:
                    stderr.write("seek failed\n")
                    sysexit(1)
            else:
                # This does not complain if over eof
                # but it will be noticed before reading
                # the first unit.
                self.file.read(skip_bytes)
        # Buffer contains input and it is read one byte at time.
        # Strategy is to zero used bits immediately.
        if self.skip_bits % 8 != 0:
            self.bits_in_buffer = 8 - self.skip_bits % 8
            self.buffer = self._read()
            self.buffer &= (1 << self.bits_in_buffer) - 1
        else:
            self.buffer = self._read()
            self.bits_in_buffer = 8

    # def units(self):
    #     """Iterator for units"""
    #     if self.eof:  ## eof while skip_bits
    #         return
    #     while True:
    #         # Exit if we got end of file in processing the previous unit
    #         # (this means that we already have used some extra zeros).
    #         if self.eof:
    #             if self.assert_aligned:
    #                 stderr.write("Non-aligned end of file\n")
    #                 sysexit(1)
    #             break
    #
    #         # Extract one unit from the buffer.
    #         # If there is not enough data in buffer,
    #         # read more data one byte at time.
    #         while self.bits_in_buffer < self.unit_size:
    #             self.buffer <<= 8
    #             self.buffer += self._read()
    #             self.bits_in_buffer += 8
    #         right_edge = self.bits_in_buffer - self.unit_size
    #         unit = self.buffer >> right_edge
    #
    #         # Yield unit if counter gives permission (skip & count).
    #         self.counter.next()
    #         if self.counter.included():
    #             if self.reverse_unit:
    #                 unit = _reverse(unit, self.unit_size)
    #             yield unit
    #         if self.counter.finished():
    #             self.eof = True
    #
    #         # Remove the extracted unit from the buffer.
    #         self.bits_in_buffer -= self.unit_size
    #         self.buffer &= (1 << self.bits_in_buffer) - 1
    #
    #         # If buffer is empty, this is an allowed end of file point
    #         # if --assert_aligend is requested.
    #         # We have to look-ahead to check the eof.
    #         if self.bits_in_buffer == 0:
    #             self.buffer = self._read()
    #             if self.eof:
    #                 break
    #             self.bits_in_buffer = 8
    #
    #         # Skip the gap.
    #         while self.bits_in_buffer < self.gap:
    #             # Overwrite, we won't need more than 8 bits.
    #             self.buffer = self._read()
    #             self.bits_in_buffer += 8
    #         self.bits_in_buffer -= self.gap
    #         assert self.bits_in_buffer <= 8
    #         self.buffer &= (1 << self.bits_in_buffer) - 1
    #
    #         # Look-ahead to detect eof
    #         if self.bits_in_buffer == 0:
    #             self.buffer = self._read()
    #             self.bits_in_buffer = 8


class MergeInputStream(FileInputStream):
    """Reads units from merge file.

    Special type of FileInputStream for merge files.

    """

    def read_bits(self, bits):
        """Read bits from merge file

        Does not care of counter or gap.

        """
        while self.bits_in_buffer < bits:
            self.buffer <<= 8
            self.buffer += self._read()
            self.bits_in_buffer += 8
        right_edge = self.bits_in_buffer - bits
        unit = self.buffer >> right_edge
        if self.reverse_unit:
            unit = _reverse(unit, self.unit_size)
        self.bits_in_buffer -= bits
        self.buffer &= (1 << self.bits_in_buffer) - 1
        return unit


#
# Counter class to help with --count and --skip
#

class Counter:
    """Counts units/tuples to be skipped

    Old interface:
    Counter is initialized on side of
    receiving units. It is initialized
    with:
    - skip: how many units should be stepped on the beginning
    - count: how many unit we take after the skip, None=unlimited

    Whenever a unit is received, we can ask from the
    counter:
    - Included - yes, we should use this unit
    - Finished - used units have finished so we do not
      need to read any more

    When we have finished using the unit, we call c.next()
    to get to the next unit.

    Only the new interface supports skip.

    New interface: Iterator that tells the same information
    returning string telling status of the stream:
    (In the order of decreasing priority)
    - "finished" - no need to call iterator again
    - "skip"/"step" (skip in beginning, step in middle)
    - "use"
    Step allows you to take every second, third etc.
    unit from the stream. Count refers to returned values,
    skipped do not count.
    """

    def __init__(self, skip=0, units=None, step=0):
        assert step >= 0 and skip >= 0
        assert units is None or units > 0
        self.current = 0
        self.skip = skip
        self.units = units
        self.step = step
        # print(f"skip={self.skip} count={self.count} step={self.step}")

    def __next__(self):
        # Current counts starts from 0 and is immediately increased
        # current == 1 means first unit
        self.current += 1

        def size_is_limited():
            return self.units is not None

        if size_is_limited():
            # ("use" + "step"...)... + "use"    # number of "use"s is self.count
            #        |
            #        -> 1 + self.step
            #                     |
            #                     -> * (self.units - 1)
            #                            |
            #                            -> + 1
            data_units_expected = ((1 + self.step) * (self.units - 1)) + 1
        else:
            data_units_expected = "unlimited"
        # print(f"{data_units_expected} units expected")
        if size_is_limited() \
                and self.current > self.skip + data_units_expected:
            return "finished"
        elif self.current <= self.skip:
            return "skip"
        elif ((self.current - self.skip - 1) % (self.step + 1)) == 0:
            return "use"
        else:
            return "step"

    def __iter__(self):
        return self

    def next(self):
        self.current += 1

    def included(self):
        if self.current <= self.skip:
            return False
        elif not self.units:
            return True
        return self.current <= (self.skip + self.units)

    def finished(self):
        if self.units:
            return self.current > self.skip + self.units
        else:
            return False
