#!/usr/bin/env python3
"""A minimal `org.kde.StatusNotifierWatcher` -- M90/T90.7, for `just
tray-drive --private-bus` (`scripts/tray_drive.sh`).

The drive's ordinary leg uses the desktop's watcher on the session bus,
which puts the item in the user's own tray for the run. With
`--private-bus` the drive runs inside `dbus-run-session` instead, on a bus
nothing else is on, and this script is the watcher there: it owns the
well-known name, keeps `RegisteredStatusNotifierItems` the way KDE's does
(the service string an item registers, prefixed with the caller's unique
name when it registered a bare path), drops an item whose owner leaves
the bus, and answers `IsStatusNotifierHostRegistered` true, as every
watcher in the wild does (`ksni`'s `service.rs` names KDE's and GNOME's).

It is a test double for the *registry*, not for the item: what the drive
asserts -- the item's dbusmenu layout, its `Event(id, "clicked")`, the
`enabled` property -- is served by the application under test and read
over D-Bus exactly as on the desktop. Needs python3-gi.

Prints `sni-watcher: ready` on stdout once the name is owned.
"""
import sys

from gi.repository import Gio, GLib

XML = """
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg name="service" type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg name="service" type="s" direction="in"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
  </interface>
</node>
"""

items = {}  # service string -> owner's unique name
conn = None


def emit(name, service):
    conn.emit_signal(None, "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher", name,
                     GLib.Variant("(s)", (service,)))


def on_call(_c, sender, _path, _iface, method, params, inv):
    if method == "RegisterStatusNotifierItem":
        (service,) = params.unpack()
        if service.startswith("/"):
            service = sender + service
        items[service] = sender
        emit("StatusNotifierItemRegistered", service)
    inv.return_value(None)


def on_get(_c, _sender, _path, _iface, prop):
    if prop == "RegisteredStatusNotifierItems":
        return GLib.Variant("as", sorted(items))
    if prop == "IsStatusNotifierHostRegistered":
        return GLib.Variant("b", True)
    return GLib.Variant("i", 0)


def on_owner_changed(_c, _sender, _path, _iface, _sig, params):
    name, _old, new = params.unpack()
    if new:
        return
    for service, owner in list(items.items()):
        if owner == name or service.split("/", 1)[0] == name:
            del items[service]
            emit("StatusNotifierItemUnregistered", service)


def main():
    global conn
    conn = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    conn.register_object("/StatusNotifierWatcher", node.interfaces[0], on_call, on_get, None)
    conn.signal_subscribe("org.freedesktop.DBus", "org.freedesktop.DBus", "NameOwnerChanged",
                          "/org/freedesktop/DBus", None, Gio.DBusSignalFlags.NONE, on_owner_changed)
    loop = GLib.MainLoop()

    def acquired(*_):
        print("sni-watcher: ready", flush=True)

    def lost(*_):
        print("sni-watcher: could not own org.kde.StatusNotifierWatcher", file=sys.stderr, flush=True)
        loop.quit()

    Gio.bus_own_name_on_connection(conn, "org.kde.StatusNotifierWatcher", Gio.BusNameOwnerFlags.NONE,
                                   acquired, lost)
    loop.run()
    return 1


if __name__ == "__main__":
    sys.exit(main())
