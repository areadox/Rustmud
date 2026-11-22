# Sicherheit

## Unterstützte Versionen

Sicherheitskorrekturen gibt es für die jeweils neueste veröffentlichte Version.

## Sicherheitslücken melden

Bitte melde Sicherheitslücken **nicht** als öffentliches Issue, sondern vertraulich über
[„Report a vulnerability“](https://github.com/areadox/Rustmud/security/advisories/new)
(GitHub Private Vulnerability Reporting).

Bitte beschreibe, wie sich das Problem nachvollziehen lässt und welche Auswirkungen es hat.
Du erhältst in der Regel innerhalb einer Woche eine Rückmeldung.

## Hinweis zu Lua-Skripten

Lua-Skripte laufen mit vollen Rechten des Benutzers (inklusive `io` und `os`), genau wie bei
MUSHclient. Lade nur Skripte, denen du vertraust.

## Hinweis zur Update-Funktion

Die Update-Prüfung fragt ausschließlich die GitHub-API dieses Repositorys über HTTPS ab
(`api.github.com/repos/areadox/Rustmud/releases/latest`) und lädt nur die dort veröffentlichte
Datei `rustmud-setup-x64.exe` herunter. Es werden keine Nutzungsdaten übertragen.
