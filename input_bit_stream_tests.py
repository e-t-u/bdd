import io
import itertools
import unittest

from input_bit_stream import *


class CounterTest(unittest.TestCase):

    def test_counter_iterator(self):
        c = Counter()
        self.assertEqual(
            ["use", "use", "use", "use", "use"],
            [i for i in itertools.islice(c, 5)],
        )

    def test_counter_iterator_skip(self):
        c = Counter(skip=2)
        self.assertEqual(
            ["skip", "skip", "use", "use", "use"],
            [i for i in itertools.islice(c, 5)],
        )

    def test_counter_iterator_count(self):
        c = Counter(skip=1, units=2)
        self.assertEqual(
            ["skip", "use", "use", "finished", "finished"],
            [i for i in itertools.islice(c, 5)],
        )

    def test_counter_iterator_step1(self):
        c = Counter(skip=1, step=1)
        self.assertEqual(
            ["skip", "use", "step", "use", "step"],
            [i for i in itertools.islice(c, 5)],
        )

    def test_counter_iterator_step_count2(self):
        c = Counter(step=1, units=2)
        self.assertEqual(
            ["use", "step", "use", "finished", "finished"],
            [i for i in itertools.islice(c, 5)],
        )

    def test_counter_iterator_step3_count(self):
        c = Counter(step=3, units=2)
        self.assertEqual(
            ["use", "step", "step", "step", "use"],
            [i for i in itertools.islice(c, 5)],
        )


class ZeroReaderTest(unittest.TestCase):

    def test_zero_reader(self):
        reader = ZeroStream()
        self.assertEqual(
            [0, 0, 0, 0, 0],
            [unit for unit in itertools.islice(reader.units(), 5)]
        )

    def test_zero_reader_counter(self):
        reader = ZeroStream(Counter(units=3000))
        self.assertEqual(
            [0 for _ in range(3000)],
            [unit for unit in reader.units()],
        )


class OneReaderTest(unittest.TestCase):

    def test_one_reader(self):
        reader = OneStream(Counter(units=3000))
        self.assertEqual(
            [255 for _ in range(3000)],
            [unit for unit in reader.units()],
        )

    def test_one_reader_strange_counters(self):
        reader = OneStream(Counter(skip=1111, units=3000, step=3))
        self.assertEqual(3000, len(list(reader.units())))

    def test_one_reader_13_bits(self):
        reader = OneStream(Counter(units=333), unit_size=13)
        one = 1 + 2 + 4 + 8 + 16 + 32 + 64 + 128 + 256 + 512 + 1024 + 2048 + 4096
        self.assertEqual(
            list(itertools.repeat(one, 333)),
            list(reader.units())
        )


class RandomReaderTest(unittest.TestCase):

    @unittest.skip("slow, undeterministic but important")
    def test_random_reader_bits(self):
        NUM = 10000
        reader = RandomStream(Counter(units=NUM), unit_size=1)
        count = {0: 0, 1: 0}
        for unit in reader.units():
            count[unit] += 1
        max_proportion = (max(count.values()) / NUM) / (1 / 2)
        # print(f"Binary units, {count} one digit max {max_proportion} more common than the other")
        self.assertGreater(
            1.02,  # Allow one binary digit to be 2% more common than the other
            max_proportion,
        )

    @unittest.skip("slow, undeterministic but important")
    def test_random_reader_bytes(self):
        NUM = 100000
        reader = RandomStream(Counter(units=NUM), unit_size=8)
        count = {}
        for i in range(256):
            count[i] = 0
        for unit in reader.units():
            count[unit] += 1
        # how much higher is max digit as compared to normal 1/256
        max_proportion = (max(count.values()) / NUM) / (1 / 256)
        # print(f"Byte units, {count}\none digit max {max_proportion} more common than the other")
        self.assertGreater(
            1.20,  # allow one digit that is 20% more common than others
            max_proportion,
        )

    @unittest.skip("slow, undeterministic but important")
    def test_random_reader_large(self):
        NUM = 10000
        reader = RandomStream(Counter(units=NUM), unit_size=234)
        sum = 0
        count = 0
        for unit in reader.units():
            sum += unit
            count += 1
        mean = sum / count
        theoretical_mean = 2 ** 234 / 2
        difference = max(mean, theoretical_mean) / min(mean, theoretical_mean)
        # print(f"234 bit random average {mean} differs {(difference - 1) * 100}% from theoretical")
        self.assertGreater(
            1.01,
            difference
        )


class CounterReaderTest(unittest.TestCase):

    def test_counter_reader(self):
        reader = CounterStream()
        self.assertEqual(
            list(itertools.islice(itertools.count(0), 100)),
            [unit for unit in itertools.islice(reader.units(), 100)]
        )

    def test_counter_reader_under_byte_wrapping(self):
        reader = CounterStream(unit_size=2)
        self.assertEqual(
            [0, 1, 2, 3, 0, 1, 2, 3],
            [unit for unit in itertools.islice(reader.units(), 8)],
        )

    def test_counter_reader_over_byte_wrapping(self):
        reader = CounterStream(Counter(skip=1022), unit_size=10)
        self.assertEqual(
            [1022, 1023, 0, 1],
            [unit for unit in itertools.islice(reader.units(), 4)],
        )

    def test_counter_reader_skips(self):
        reader = CounterStream(Counter(skip=1, units=3, step=2))
        self.assertEqual(
            [1, 4, 7],
            [unit for unit in reader.units()],
        )


class IntegerReaderTest(unittest.TestCase):

    def test_integer_reader(self):
        reader = IntegerInputStream(fd=io.StringIO("12\n34\n56"))
        self.assertEqual(
            [12, 34, 56],
            [unit for unit in itertools.islice(reader.units(), 3)]
        )

    def test_integer_reader_whitespace(self):
        reader = IntegerInputStream(fd=io.StringIO("""
# Comments and empty lines allowed
211

34
42
"""))
        self.assertEqual(
            [211, 34, 42],
            [unit for unit in itertools.islice(reader.units(), 3)]
        )

    def test_integer_reader_unit_size_over_byte(self):
        reader = IntegerInputStream(unit_size=9, fd=io.StringIO("""
            456
            511
            512
            1024
        """))
        self.assertEqual(
            [456, 511, 0, 0],
            [unit for unit in itertools.islice(reader.units(), 4)]
        )

    def test_integer_reader_unit_size_under_byte(self):
        reader = IntegerInputStream(unit_size=7, fd=io.StringIO("""
            23
            127
            129
        """))
        self.assertEqual(
            [23, 127, 1],
            [unit for unit in itertools.islice(reader.units(), 3)]
        )

    def test_integer_reader_errors(self):
        reader = IntegerInputStream(fd=io.StringIO("""
             xx
             -5
             257
         """))
        self.assertEqual(
            [0, 5, 1],
            [unit for unit in itertools.islice(reader.units(), 3)]
        )

    def test_integer_reader_counter(self):
        reader = IntegerInputStream(Counter(skip=1, step=1, units=3), fd=io.StringIO("""
            1
            2
            3
            4
            5
            6
            7
            8
        """))
        self.assertEqual(
            [2, 4, 6],
            [unit for unit in reader.units()]
        )


class FileReaderTest(unittest.TestCase):

    def test_file_reader(self):
        reader = FileInputStream(
            fd=io.BytesIO(b"\x02\x03\x04"),
        )
        self.assertEqual(
            [2, 3, 4],
            [unit for unit in reader.units()]
        )

    def test_file_reader_bits(self):
        reader = FileInputStream(
            unit_size=1,
            fd=io.BytesIO(b"\x5A"),
        )
        self.assertEqual(
            [0, 1, 0, 1, 1, 0, 1, 0],
            [unit for unit in reader.units()]
        )

    def test_file_reader_bit_boundaries(self):
        reader = FileInputStream(
            skip_bits=1,
            fd=io.BytesIO(b"\x55\x55\x55"),
        )
        self.assertEqual(
            [0xAA, 0xAA],    # last is incomplete and ignored by default
            [unit for unit in reader.units()]
        )



if __name__ == '__main__':
    unittest.main()
