-- Beispielskript für rustmud
-- In den Welt-Einstellungen unter „Lua-Datei“ eintragen.

-- Wird nach dem Verbinden aufgerufen
function OnConnect()
  ColourNote("lime", "", "Verbunden mit ", "white", "", WorldName())
end

function OnDisconnect()
  Note("Verbindung beendet.")
end

-- Wird bei jedem GMCP-Paket aufgerufen; die Daten liegen zusätzlich in der Tabelle gmcp
function OnGMCP(paket, json)
  if paket == "Char.Vitals" and gmcp.Char.Vitals.lp then
    local lp = gmcp.Char.Vitals.lp
    if lp < 30 then
      ColourNote("red", "", "Achtung, nur noch " .. lp .. " LP!")
    end
  end
end

-- Trigger-Funktion: im Trigger-Editor unter „Lua-Funktion“ eintragen
-- Muster z. B.: * kommt herein.
function begruessen(name, zeile, wildcards)
  Send("winke " .. wildcards[1])
end

-- Direkt in der Eingabezeile ausführbar mit dem Lua-Präfix, z. B.:
--   /Note(1 + 2)
--   /SetVariable("ziel", "ork")
