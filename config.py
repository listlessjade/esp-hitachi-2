# from whiptail import Whiptail
from dialog import Dialog
import functools
from abc import ABC, abstractmethod
import typing
import json
import os.path


class Input(ABC):
    @abstractmethod
    def run(self, w: Dialog):
        pass

    @abstractmethod
    def get_value(self):
        pass

    @abstractmethod
    def set_value(self, value):
        pass

    @abstractmethod
    def get_description(self):
        pass

    @abstractmethod
    def get_display_name(self):
        pass


class RangeInput(Input):
    def __init__(
        self, question: str, rng: typing.Tuple[int, int], default=0, description="", display_name=None
    ):
        self.question = question
        self.description = description
        self.value = default
        self.range = rng
        self.display_name = display_name or question

    def run(self, w: Dialog):
        self.value = w.rangebox(
            self.question, min=self.range[0], max=self.range[1], init=self.value
        )[1]

    def get_value(self):
        return self.value
    
    def set_value(self, value):
        self.value = value

    def get_description(self):
        return self.description
    
    def get_display_name(self):
        return self.display_name


class BoolInput(Input):
    def __init__(self, question: str, description="", default=True, display_name=None):
        self.question = question
        self.description = description
        self.value = default
        self.display_name = display_name or question

    def run(self, w):
        self.value = (
            w.yesno(
                f"{self.question} (currently: {'enabled' if self.value else 'disabled'})"
            )
            == Dialog.OK
        )

    def get_value(self):
        return self.value
    
    def set_value(self, value):
        self.value = value

    def get_description(self):
        return self.description
    
    def get_display_name(self):
        return self.display_name


class StrInput(Input):
    def __init__(self, question: str, description="", display_name=None, as_int=False):
        self.description = description
        self.question = question
        self.value = ""
        self.display_name = display_name or question
        self.as_int = as_int

    def run(self, w):
        self.value = w.inputbox(self.question, init=str(self.value))[1]
        if self.as_int:
            self.value = int(self.value)

    def get_value(self):
        return self.value
    
    def set_value(self, value):
        self.value = value

    def get_description(self):
        return self.description

    def get_display_name(self):
        return self.display_name

class RadioList(Input):
    def __init__(self, title, options, description="", default="", display_name=None):
        self.options = options
        self.title = title
        self.description = description
        self.value = default
        self.display_name = display_name or title
        # super().__init__()
        # pass

    def run(self, w):
        self.value = w.radiolist(self.title, choices=[(k,v,k==self.value) for k,v in self.options])[1]

    def get_value(self):
        return self.value
    
    def set_value(self, value):
        self.value = value

    def get_description(self):
        return self.description
    
    def get_display_name(self):
        return self.display_name

class Menu(Input):
    def __init__(
        self, title: str, items: typing.Dict[str, Input], default_values={}, return_text="Return", display_name=None
    ):
        self.opts = items
        self.title = title
        self.return_text = return_text
        self.display_name = display_name or title
        self.names_to_tags = {v.get_display_name(): k for k, v in self.opts.items()}
        print(self.names_to_tags)

    def add_str(self, key: str, question: str, description=None, display_name=None):
        self.opts[key] = StrInput(question, description, display_name=display_name)
        self.names_to_tags = {v.get_display_name(): k for k, v in self.opts.items()}

    def add_menu(
        self, key: str, title: str, items: typing.Dict[str, Input], return_text="Return", display_name=None
    ):
        self.opts[key] = Menu(title, items, return_text=return_text, display_name=display_name)
        self.names_to_tags = {v.get_display_name(): k for k, v in self.opts.items()}

    def add_input(self, key: str, input: Input):
        self.opts[key] = input
        self.names_to_tags = {v.get_display_name(): k for k, v in self.opts.items()}

    def selections(self):
        opts = [(v.get_display_name(), v.get_description()) for k, v in self.opts.items()]
        return opts

    def run(self, w):
        while True:
            [code, selected] = w.menu(self.title, choices=self.selections())
            if code == Dialog.CANCEL:
                break

            self.opts[self.names_to_tags[selected]].run(w)

    def get_value(self):
        return {k: v.get_value() for k, v in self.opts.items()}

    def set_value(self, value):
        for key, value in value.items():
            self.opts[key].set_value(value)

    def get_description(self):
        return self.title
    
    def get_display_name(self):
        return self.display_name


cfg = Menu("Config", {})
cfg.add_menu(
    "wifi",
    "WiFi Options",
    {
        "enable": BoolInput("Enable WiFi?"),
        "ssid": StrInput("SSID"),
        "password": StrInput("Password"),
        "auth": RadioList(
            "Auth Method",
            [
                ("WPA2_PERSONAL", "WPA2 Personal Auth"),
                ("WPA2_ENTERPRISE", "FANCY"),
            ],
            default="WPA2_PERSONAL"
        ),
        "identity": StrInput("Identity", description="(for WPA2 Enterprise)"),
        "username": StrInput("Username", description="(for WPA2 Enterprise)")
    },
    display_name="WiFi"
)

cfg.add_menu(
    "motor",
    "Motor options!",
    {
        "max_power": RangeInput(
            "Maximum motor power (0-100)",
            (0, 100),
            100,
            description="Highest power level available",
        ),
        "min_power": RangeInput(
            "Minimum motor power (0-100)",
            (0, 100),
            50,
            description="Lowest power level available",
        ),
    },
    display_name="Motor"
)

cfg.add_menu(
    "remote_log",
    "Network Logging Options",
    {
        "enable": BoolInput("Enable Remote Logging?"),
        "port": StrInput("Port", description="Port to listen on for remote logging", as_int=True),
    },
    display_name="Remote Logging"
)

if os.path.exists("hitachi-config.json"):
    with open("hitachi-config.json") as f:
        cfg.set_value(json.load(f))

w = Dialog()
cfg.run(w)

print(cfg.get_value())

with open("hitachi-config.json", "w") as f:
    json.dump(cfg.get_value(), f)