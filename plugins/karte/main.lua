-- Karte – mitgeliefertes Plugin für rustmud
--
-- Viele MUDs zeigen links neben der Raumbeschreibung eine kleine Karte:
--
--   +-------------+ Hier bist Du nun also am Eingang der ...
--   |#(N)-[X]-( )-| Atem. Der Tunnel, der vor Dir liegt ...
--   +-------------+ von Wasser.
--
-- Das Plugin schneidet den Kartenblock heraus, zeigt ihn farbig im Fenster
-- „Karte“ und lässt nur den Text in der Ausgabe stehen. Breite und Höhe
-- werden bei jeder Karte neu aus dem Rahmen bestimmt.
--
-- Befehl:  karte breite <zahl>  → maximale Kartenbreite (Standard 60).
--          Breitere Rahmen gelten als Tabelle und bleiben in der Ausgabe.

local FENSTER = "karte"
local zeilen = nil      -- Kartenzeilen, während eine Karte empfangen wird
local original = {}     -- zurückgehaltene Originalzeilen (falls es doch keine Karte ist)
local breite = 0
local gezeigt = false

local function max_breite()
  return tonumber(GetVariable("max_breite")) or 60
end

-- Breite eines Rahmens „+-----+“ am Zeilenanfang, sonst nil
local function rahmen(plain)
  local striche = plain:match("^%+(%-+)%+")
  if not striche then return nil end
  local w = #striche + 2
  if w < 5 or w > max_breite() then return nil end
  local danach = plain:sub(w + 1, w + 1)
  if danach ~= "" and danach ~= " " then return nil end -- z. B. Tabelle +---+---+
  return w
end

local function anzeigen(liste)
  if #liste < 2 then return end
  WindowClear(FENSTER)
  for _, z in ipairs(liste) do
    WindowAddLine(FENSTER, z)
  end
  if not gezeigt then
    WindowShow(FENSTER)
    gezeigt = true
  end
end

-- Text rechts neben der Karte (ohne das trennende Leerzeichen); "" = Zeile ausblenden
local function resttext(ansi, w)
  local rest = AnsiSub(ansi, w + 2)
  if AnsiStrip(rest):match("^%s*$") then return "" end
  return rest
end

function OnPluginInstall()
  WindowCreate(FENSTER, "🗺 Karte")
  WindowOption(FENSTER, "fit", true)
end

-- Enthält der Kartenteil echte Wörter (≥ 3 Buchstaben)? Dann ist es eine Tabelle oder ein Kasten.
-- Folgen aus o/O/x/X (z. B. Bäume „oOo“) und einzelne Buchstaben wie (N) gelten als Kartenzeichen.
local function hat_woerter(s)
  for wort in s:gmatch("%a+") do
    if #wort >= 3 and not wort:match("^[oOxX]+$") then return true end
  end
  return false
end

-- Merkt sich die Originalzeile; ausgeblendete Zeilen werden bei einem Irrtum nachgereicht
local function merken(ansi, rest)
  table.insert(original, { ansi = ansi, ausgeblendet = (rest == "") })
  return rest
end

-- War doch keine Karte (z. B. eine Tabelle): ausgeblendete Zeilen unverändert ausgeben
local function verwerfen()
  for _, o in ipairs(original) do
    if o.ausgeblendet then EmitLine(o.ansi) end
  end
  zeilen, original = nil, {}
end

function OnPluginLineReceived(plain, ansi)
  if zeilen == nil then
    local w = rahmen(plain)
    if not w then return true end
    breite = w
    zeilen, original = { AnsiSub(ansi, 1, w) }, {}
    -- neuer Raum: Seitenansicht beginnt hier
    SetPageStart()
    return merken(ansi, resttext(ansi, w))
  end

  local w = breite
  local karte = plain:sub(1, w)
  local ist_zeile = karte:sub(1, 1) == "|" and karte:sub(w, w) == "|"
      and (#plain == w or plain:sub(w + 1, w + 1) == " ")
  local ist_ende = rahmen(plain) == w

  -- Kartenzeilen enthalten keine Wörter – sonst ist es eine Tabelle oder ein Kasten
  if ist_zeile and hat_woerter(karte) then
    verwerfen()
    return true
  end

  if not ist_zeile and not ist_ende then
    -- Karte ohne Abschlussrahmen: bisherigen Stand zeigen und Zeile normal behandeln
    if #zeilen >= 3 then
      anzeigen(zeilen)
      zeilen, original = nil, {}
    else
      verwerfen()
    end
    return OnPluginLineReceived(plain, ansi)
  end

  table.insert(zeilen, AnsiSub(ansi, 1, w))
  if ist_ende then
    if #zeilen >= 3 then
      anzeigen(zeilen)
      zeilen, original = nil, {}
    else
      verwerfen()
      return true
    end
  end
  return merken(ansi, resttext(ansi, w))
end

function OnPluginCommand(befehl)
  local zahl = befehl:match("^karte%s+breite%s+(%d+)$")
  if zahl then
    SetVariable("max_breite", zahl)
    Note("Karte: maximale Breite ist jetzt " .. zahl .. " Zeichen.")
    return false
  end
  if befehl == "karte" then
    WindowShow(FENSTER)
    return false
  end
  return true
end
