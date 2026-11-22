-- Spielstand – mitgeliefertes Plugin für rustmud
--
-- Erkennt die Spielstand-Tabelle (Rahmen +-----+, Zeilen | ... |) mit
-- Trefferpunkten, Eigenschaften usw., blendet sie in der Ausgabe aus und zeigt
-- sie im Fenster „Spielstand“ mit farbigen Balken an. Andere Tabellen bleiben
-- unverändert in der Ausgabe.
--
-- Befehle:
--   spielstand              Fenster zeigen und „sp“ senden
--   spielstand auto <sek>   alle <sek> Sekunden automatisch „sp“ senden (aus: 0)
--   spielstand ausgabe an   Tabelle zusätzlich in der Ausgabe lassen (aus: ausgabe aus)

local FENSTER = "spielstand"
local block = nil         -- gesammelte Tabellenzeilen
local ruhe = 0            -- Ticks seit der letzten Tabellenzeile
local stand = nil         -- zuletzt erkannter Spielstand
local live = {}           -- Werte aus GMCP
local gezeigt = false
local naechstes_sp = nil

local FARBEN = {
  ["Treffer"] = "#d9534f", ["Leben"] = "#d9534f", ["hp"] = "#d9534f",
  ["Aktion"] = "#e0a030", ["Ausdauer"] = "#e0a030",
  ["Zauber"] = "#4fb3d9", ["sp"] = "#4fb3d9",
  ["Mana"] = "#8a6de9", ["mp"] = "#8a6de9",
  ["Tragkraft"] = "#8c8c8c",
}

local function farbe(name)
  for k, f in pairs(FARBEN) do
    if name:lower():find(k:lower(), 1, true) == 1 then return f end
  end
  return "#5fa85f"
end

local function trim(s) return (s:match("^%s*(.-)%s*$")) end

local function ausgabe_behalten() return GetVariable("ausgabe") == "an" end

-- ---------------------------------------------------------------- Auswerten

local function auswerten(zeilen)
  local s = { name = nil, balken = {}, eigenschaften = {}, details = {} }
  for _, z in ipairs(zeilen) do
    local p = z.plain
    if p:sub(1, 1) == "|" then
      local inhalt = p:match("^|(.*)|%s*$") or p:sub(2)
      for zelle in (inhalt .. "|"):gmatch("([^|]*)|") do
        local c = trim(zelle)
        if c == "" or c:match("^[%d%%%s]+$") then
          -- leer oder Prozent-Skala
        elseif c:match("^[%a]+:%s+%-?%d+%s*%(%s*%-?%d+%)") then
          local name, wert, max = c:match("^([%a]+):%s+(%-?%d+)%s*%(%s*(%-?%d+)%)")
          table.insert(s.balken, { name = name, wert = tonumber(wert), max = tonumber(max) })
        elseif c:match("^Tragkraft%s+%d+%%") then
          table.insert(s.balken, { name = "Tragkraft", wert = tonumber(c:match("(%d+)%%")), max = 100 })
        elseif c:find("%.%.%.") then
          for name, wert in c:gmatch("([^%s%.:]+)%.*:%s*(%-?%d+)") do
            table.insert(s.eigenschaften, { name, wert })
          end
        elseif c:find(":") then
          local k, v = c:match("^(.-):%s*(.*)$")
          table.insert(s.details, { trim(k), trim(v) })
        elseif s.name == nil then
          s.name = c
        else
          local k, v = c:match("^(%S+)%s+(.+)$")
          if k then table.insert(s.details, { k, v }) else table.insert(s.details, { c, "" }) end
        end
      end
    end
  end
  return s
end

local function ist_spielstand(zeilen)
  local text = ""
  for _, z in ipairs(zeilen) do text = text .. z.plain .. "\n" end
  return text:find("Trefferpunkte") ~= nil or text:find("Lebenspunkte") ~= nil
end

-- ---------------------------------------------------------------- Zeichnen

local function zeichnen()
  WindowClear(FENSTER)
  if stand then
    WindowHeading(FENSTER, stand.name or "Spielstand")
    for _, b in ipairs(stand.balken) do
      WindowGauge(FENSTER, b.name, b.wert, b.max, farbe(b.name))
    end
  end
  local hat_live = next(live) ~= nil
  if hat_live then
    WindowHeading(FENSTER, "Live (GMCP)")
    local namen = {}
    for k in pairs(live) do table.insert(namen, k) end
    table.sort(namen)
    for _, k in ipairs(namen) do
      WindowGauge(FENSTER, k, live[k].wert, live[k].max, farbe(k))
    end
  end
  if stand then
    if #stand.eigenschaften > 0 then
      WindowSeparator(FENSTER)
      WindowHeading(FENSTER, "Eigenschaften")
      for _, e in ipairs(stand.eigenschaften) do WindowPair(FENSTER, e[1], e[2]) end
    end
    if #stand.details > 0 then
      WindowSeparator(FENSTER)
      WindowHeading(FENSTER, "Details")
      for _, d in ipairs(stand.details) do WindowPair(FENSTER, d[1], d[2]) end
    end
    WindowSeparator(FENSTER)
    WindowPair(FENSTER, "Aktualisiert", stand.zeit)
  elseif not hat_live then
    WindowAddLine(FENSTER, "Noch kein Spielstand – gib „sp“ ein.")
  end
  if not gezeigt then
    WindowShow(FENSTER)
    gezeigt = true
  end
end

local function abschliessen()
  local zeilen = block
  block = nil
  if ist_spielstand(zeilen) then
    stand = auswerten(zeilen)
    stand.zeit = os.date("%H:%M:%S")
    zeichnen()
    if ausgabe_behalten() then
      for _, z in ipairs(zeilen) do EmitLine(z.ansi) end
    end
  else
    -- war eine andere Tabelle: unverändert ausgeben
    for _, z in ipairs(zeilen) do EmitLine(z.ansi) end
  end
end

-- ---------------------------------------------------------------- Callbacks

function OnPluginInstall()
  WindowCreate(FENSTER, "📊 Spielstand")
end

function OnPluginLineReceived(plain, ansi)
  if block then
    if plain:match("^[%+|]") then
      table.insert(block, { plain = plain, ansi = ansi })
      ruhe = 0
      return false
    end
    abschliessen()
  end
  -- Beginn einer breiten Tabelle: Rahmenzeile ohne Text daneben
  if #plain >= 30 and plain:match("^%+[%-%+]+%+%s*$") then
    block = { { plain = plain, ansi = ansi } }
    ruhe = 0
    return false
  end
  return true
end

function OnPluginTick()
  -- Tabelle ohne folgende Zeile (kein Prompt): nach kurzer Pause abschließen
  if block then
    ruhe = ruhe + 1
    if ruhe >= 5 then abschliessen() end
  end
  local auto = tonumber(GetVariable("auto")) or 0
  if auto > 0 then
    local jetzt = os.time()
    naechstes_sp = naechstes_sp or (jetzt + auto)
    if jetzt >= naechstes_sp then
      naechstes_sp = jetzt + auto
      Send("sp")
    end
  end
end

function OnPluginGMCP(paket, json)
  if not paket:match("^Char") then return end
  local t = gmcp
  for teil in paket:gmatch("[^%.]+") do
    t = t and t[teil]
  end
  if type(t) ~= "table" then return end
  local geaendert = false
  for k, v in pairs(t) do
    if type(v) == "number" then
      local max = t["max" .. k] or t[k .. "max"] or t["max_" .. k] or t[k .. "_max"]
      if type(max) == "number" then
        live[k] = { wert = v, max = max }
        geaendert = true
      end
    end
  end
  if geaendert then zeichnen() end
end

function OnPluginCommand(befehl)
  if befehl == "spielstand" then
    gezeigt = false
    zeichnen()
    Send("sp")
    return false
  end
  local sek = befehl:match("^spielstand%s+auto%s+(%d+)$")
  if sek then
    SetVariable("auto", sek)
    naechstes_sp = nil
    Note(sek == "0" and "Spielstand: automatische Abfrage aus."
      or ("Spielstand: alle " .. sek .. " Sekunden automatisch „sp“."))
    return false
  end
  local modus = befehl:match("^spielstand%s+ausgabe%s+(%a+)$")
  if modus then
    SetVariable("ausgabe", modus)
    Note("Spielstand: Tabelle in der Ausgabe " .. (modus == "an" and "behalten." or "ausblenden."))
    return false
  end
  return true
end
