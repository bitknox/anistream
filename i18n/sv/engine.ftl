notice-saved = Sparade { $label }.
notice-not-saved = Det gick inte att spara inställningen. { $reason }
notice-title-load-failed = Det gick inte att läsa in titeln. Kontrollera anslutningen och försök igen.
notice-playback-failed = Uppspelningen misslyckades. Prova en annan { -source }.
notice-engine-fault = Något gick fel i { -app-name }. Appen fortsatte, men om en skärm verkar fast avslutar du och öppnar den igen.
notice-database-set-aside = Databasen var skadad, så { -app-name } startade en ny. Den gamla filen sparas på { $path }. Dina { -tracker-pl } hämtar tillbaka det de har när synken körs igen.
start-failed-folder = { -app-name } kunde inte skapa mappen { $path }. Kontrollera att disken har plats och att mappen går att skriva i, och försök igen. { $reason }
start-failed-config = { $path } har en inställning som { -app-name } inte kan använda: { $problem }. Rätta till den eller ta bort den och försök igen.
start-failed-database = { -app-name } kunde inte öppna sin databas på { $path }. Om en annan app från { -app-name } är öppen i den här mappen avslutar du den och försöker igen. { $reason }
start-failed-network = { -app-name } kunde inte skapa sin nätverksanslutning med nätverksinställningarna i config.toml. { $reason }
start-failed = { -app-name } kunde inte starta. Avsluta och öppna den igen. { $reason }
start-failed-remote = Fjärrkontrollen kunde inte starta. { $reason }
start-failed-player = Spelaren kunde inte starta. { $reason }
notice-episodes-queued = { $count ->
    [one] Lade till ett avsnitt i kön.
   *[other] Lade till { $count } avsnitt i kön.
 }
