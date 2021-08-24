import json
from html.parser import HTMLParser
from enum import Enum

class CurrentTab(Enum):
    Unknown = 0
    coverFile = 1
    coverPerLo = 2
    headerCovTableEntryLo = 3
    Skip = 4

class MyHTMLParser(HTMLParser):
    def __init__(self, history):
        super().__init__()
        self.current = CurrentTab.Unknown
        self.history = history
        self.current_file = None

    def handle_starttag(self, tag, attrs):
        if tag == 'td':
            attrs = dict(attrs)

            try:
                if attrs['class'] == 'coverFile':
                    self.current = CurrentTab.coverFile

                if self.current != CurrentTab.Skip and \
                    attrs['class'] == 'headerCovTableEntryLo':
                    self.current = CurrentTab.headerCovTableEntryLo

                if self.current != CurrentTab.Skip and \
                    attrs['class'] == 'coverPerLo':
                    self.current = CurrentTab.coverPerLo
            except:
                pass

    def handle_data(self, data):
        if self.current == CurrentTab.coverFile:
            self.current = CurrentTab.Unknown
            self.current_file = data

            if data not in self.history:
                self.history[data] = []

        if self.current == CurrentTab.headerCovTableEntryLo:
            self.history['total coverage'].append(float(data.rstrip('\xa0%')))
            self.current = CurrentTab.Skip

        if self.current == CurrentTab.coverPerLo:
            self.history[self.current_file].append(float(data.rstrip('\xa0%')))
            self.current = CurrentTab.Skip

try:
    with open('./history.json', 'r') as fp:
        history = json.load(fp)
except:
    history = { 'total coverage': [] }

parser = MyHTMLParser(history)
parser.feed(open('./index.html','r').read())

with open('./history.json', 'w') as fp:
    json.dump(history, fp)
