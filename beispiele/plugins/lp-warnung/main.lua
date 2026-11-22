-- LP-Warnung – Beispiel-Plugin für rustmud
--
-- Warnt, wenn die Lebenspunkte unter eine Grenze fallen. Funktioniert mit
-- GMCP (Char.Vitals mit "lp"/"hp") oder mit Textzeilen wie „LP: 25“.
-- Befehl:  lpwarnung 30   → Grenze auf 30 setzen
--          lpwarnung      → aktuelle Grenze anzeigen

local function grenze()
  return tonumber(GetVariable("grenze")) or 30
end

local zuletzt_gewarnt = nil

local function pruefe(lp)
  lp = tonumber(lp)
  if not lp then return end
  if lp < grenze() then
    if zuletzt_gewarnt ~= lp then
      ColourNote("white", "red", " ACHTUNG ", "red", "", " Nur noch " .. lp .. " LP!")
      zuletzt_gewarnt = lp
    end
  else
    zuletzt_gewarnt = nil
  end
end

function OnPluginInstall()
  ColourNote("silver", "", "LP-Warnung aktiv (Grenze " .. grenze() .. ", ändern mit „lpwarnung <zahl>“).")
end

function OnPluginGMCP(paket, json)
  if paket == "Char.Vitals" then
    local v = gmcp.Char.Vitals
    pruefe(v.lp or v.hp)
  end
end

function OnPluginLineReceived(zeile)
  local lp = zeile:match("LP:%s*(%d+)")
  if lp then pruefe(lp) end
  return true
end

function OnPluginCommand(befehl)
  local arg = befehl:match("^lpwarnung%s*(.*)$")
  if not arg then return true end
  if tonumber(arg) then
    SetVariable("grenze", arg)
  end
  Note("LP-Warnung: Grenze ist " .. grenze() .. " LP.")
  return false -- Befehl nicht an den MUD senden
end
