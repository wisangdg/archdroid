#!/usr/bin/env python3
# Minimal GTK3 XDND source: a small window that drags one file as text/uri-list.
import sys, pathlib, gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, Gdk
uri = pathlib.Path(sys.argv[1]).resolve().as_uri()
w = Gtk.Window(title="dragsrc")
w.set_default_size(110, 90)
w.move(1160, 20)
box = Gtk.EventBox()
box.add(Gtk.Label(label=pathlib.Path(sys.argv[1]).name))
box.drag_source_set(Gdk.ModifierType.BUTTON1_MASK, [], Gdk.DragAction.COPY)
box.drag_source_add_uri_targets()
box.connect("drag-data-get", lambda wd, ctx, data, info, t: data.set_uris([uri]))
box.connect("drag-end", lambda *a: print("drag-end", flush=True))
w.add(box)
w.connect("destroy", Gtk.main_quit)
w.show_all()
Gtk.main()
