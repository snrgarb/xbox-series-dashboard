// Cross-checks crates/playnite against the real thing: LiteDB 4.1.4, the version Playnite 10 keeps its library in, with
// Playnite's own models (PlayniteSDK) and its mapper's settings (Playnite's GameDatabase.GetCollectionMapper and
// GamesCollection.MapLiteDbEntities). Every game and preset here is made up. docs/import.md, "How sure", says what it
// showed. Run with the .NET SDK:
//
//   dotnet run -- write <dir>     a made-up Playnite library folder (games.db, filterpresets.db), written by LiteDB:
//                                 crates/playnite/tests/litedb4/ is one, read by the port's reader in its tests
//   dotnet run -- dump <file.db>  every collection and document of a LiteDB 4 file, as LiteDB itself reads it, as JSON
//                                 lines: for a file the port's writer made (`standin make-playnite`)
//   dotnet run -- touch <file.db> opens a file the port's writer made as Playnite does, and inserts, finds, updates
//                                 and deletes a game in it, as Playnite would: on a copy
using LiteDB;
using Playnite.SDK.Models;

static class Program
{
    static readonly Guid Steam = Guid.Parse("CB91DFC9-B977-43BF-8E70-55F46E410FAB");
    static readonly Guid Epic = Guid.Parse("00000002-DBD1-46C6-B5D0-B1BA559D10E4");
    // Made-up ids, chosen so that their bytes on disk (Guid.ToByteArray's order) differ from their text's.
    static readonly Guid Kestrel = Guid.Parse("4a1b2c3d-5e6f-4789-9abc-def012345678");
    static readonly Guid Couch = Guid.Parse("11223344-5566-4778-899a-abbccddeeff0");
    static readonly Guid CouchPreset = Guid.Parse("a0b1c2d3-e4f5-4a6b-8c7d-8e9fa0b1c2d3");
    static readonly Guid InstalledPreset = Guid.Parse("0f1e2d3c-4b5a-4968-8778-695a4b3c2d1e");

    static int Main(string[] args)
    {
        if (args.Length != 2)
        {
            Console.Error.WriteLine("usage: playnite-litedb write <dir> | dump <file.db> | touch <file.db>");
            return 2;
        }
        switch (args[0])
        {
            case "write": Write(args[1]); return 0;
            case "dump": Dump(args[1]); return 0;
            case "touch": Touch(args[1]); return 0;
            default: Console.Error.WriteLine($"no command {args[0]}"); return 2;
        }
    }

    // As Playnite's GameDatabase.GetCollectionMapper, with the mappings of the two collections read here.
    static BsonMapper Mapper()
    {
        var mapper = new BsonMapper
        {
            SerializeNullValues = false,
            TrimWhitespace = false,
            EmptyStringToNull = true,
            IncludeFields = false,
            IncludeNonPublic = false
        };
        mapper.RegisterType<ReleaseDate>(date => date.Serialize(), bson => ReleaseDate.Deserialize(bson.AsString));
        mapper.Entity<Game>()
            .Id(a => a.Id, false)
            .Ignore(a => a.Genres).Ignore(a => a.Developers).Ignore(a => a.Publishers).Ignore(a => a.Tags)
            .Ignore(a => a.Features).Ignore(a => a.Categories).Ignore(a => a.Platforms).Ignore(a => a.Series)
            .Ignore(a => a.AgeRatings).Ignore(a => a.Regions).Ignore(a => a.Source).Ignore(a => a.ReleaseYear)
            .Ignore(a => a.UserScoreRating).Ignore(a => a.CommunityScoreRating).Ignore(a => a.CriticScoreRating)
            .Ignore(a => a.UserScoreGroup).Ignore(a => a.CommunityScoreGroup).Ignore(a => a.CriticScoreGroup)
            .Ignore(a => a.LastActivitySegment).Ignore(a => a.AddedSegment).Ignore(a => a.ModifiedSegment)
            .Ignore(a => a.PlaytimeCategory).Ignore(a => a.IsCustomGame).Ignore(a => a.InstallationStatus);
        mapper.Entity<FilterPreset>().Id(a => a.Id, false);
        return mapper;
    }

    // How Playnite opens each collection's file (ItemCollection.InitializeCollection).
    static LiteDatabase Open(string path) => new LiteDatabase($"Filename={path};Mode=Exclusive;Cache Size=0", Mapper());

    static void Write(string dir)
    {
        Directory.CreateDirectory(dir);
        var utc = DateTimeKind.Utc;
        using (var db = Open(Path.Combine(dir, "games.db")))
        {
            var games = db.GetCollection<Game>();
            games.EnsureIndex(a => a.Id, true);
            games.Insert(new Game("Kestrel Bay")
            {
                Id = Kestrel,
                PluginId = Steam,
                GameId = "9200010",
                Playtime = 7260,
                PlayCount = 3,
                LastActivity = new DateTime(2026, 5, 1, 12, 0, 0, utc),
                Added = new DateTime(2025, 1, 2, 3, 4, 5, utc),
                Favorite = true,
                IsInstalled = true,
                CategoryIds = new List<Guid> { Couch },
                ReleaseDate = new ReleaseDate(2019, 5, 4),
            });
            games.Insert(new Game("Lowland Signal") { Id = Id(2), PluginId = Steam, GameId = "9200020", Hidden = true });
            // Longer than a page: its document goes in extend pages.
            games.Insert(new Game("Paper Lighthouse")
            {
                Id = Id(3),
                PluginId = Steam,
                GameId = "9200030",
                Playtime = 60,
                Description = string.Concat(Enumerable.Repeat("A lighthouse folded from paper. ", 300)),
            });
            games.Insert(new Game("Kestrel Bay") { Id = Id(4), PluginId = Epic, GameId = "kestrelbay", Playtime = 600 });
            games.Insert(new Game("By Hand") { Id = Id(5), PluginId = Guid.Empty, Playtime = 120 });
            // Enough more for more than one index page and data page.
            for (var n = 0; n < 60; n++)
            {
                games.Insert(new Game($"Filler {n:00}") { Id = Id(100 + n), PluginId = Steam, GameId = $"92010{n:00}", Playtime = (ulong)n * 60 });
            }
            // As a library lives: a game removed, and one played since.
            games.Delete(Id(107));
            var played = games.FindById(Id(108));
            played.Playtime = 99_999;
            games.Update(played);
        }
        using (var db = Open(Path.Combine(dir, "filterpresets.db")))
        {
            var presets = db.GetCollection<FilterPreset>();
            presets.EnsureIndex(a => a.Id, true);
            presets.Insert(new FilterPreset
            {
                Id = CouchPreset,
                Name = "Couch",
                ShowInFullscreeQuickSelection = true,
                SortingOrder = SortOrder.LastActivity,
                SortingOrderDirection = SortOrderDirection.Descending,
                Settings = new FilterPresetSettings { Category = new IdItemFilterItemProperties(Couch) },
            });
            presets.Insert(new FilterPreset
            {
                Id = InstalledPreset,
                Name = "Installed Steam",
                ShowInFullscreeQuickSelection = true,
                SortingOrder = SortOrder.Name,
                SortingOrderDirection = SortOrderDirection.Ascending,
                Settings = new FilterPresetSettings { IsInstalled = true, Library = new IdItemFilterItemProperties(Steam) },
            });
            // A preset is shown in Fullscreen's quick selection unless the user says not.
            presets.Insert(new FilterPreset
            {
                Id = Id(9),
                Name = "Not flagged",
                ShowInFullscreeQuickSelection = false,
                Settings = new FilterPresetSettings { Favorite = true },
            });
            var settings = db.GetCollection<FilterPresetsSettings>();
            settings.Insert(new FilterPresetsSettings { SortingOrder = new List<Guid> { InstalledPreset, CouchPreset } });
        }
        Console.WriteLine($"wrote {dir}");
    }

    static Guid Id(int n) => new Guid(n, 0x1234, 0x4567, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67);

    static void Dump(string path)
    {
        using var db = new LiteDatabase($"Filename={path};Mode=ReadOnly");
        foreach (var name in db.GetCollectionNames().OrderBy(n => n, StringComparer.Ordinal))
        {
            foreach (var doc in db.GetCollection(name).FindAll())
            {
                Console.WriteLine($"{name} {JsonSerializer.Serialize(doc, false, true)}");
            }
        }
    }

    static void Touch(string path)
    {
        using var db = Open(path);
        if (db.CollectionExists("FilterPreset"))
        {
            var presets = db.GetCollection<FilterPreset>();
            presets.Insert(new FilterPreset { Id = Guid.NewGuid(), Name = "Touched", Settings = new FilterPresetSettings() });
            Console.WriteLine($"{presets.Count()} presets: {string.Join(", ", presets.FindAll().Select(p => p.Name))}");
            var order = db.GetCollection<FilterPresetsSettings>().FindAll().First().SortingOrder;
            Console.WriteLine($"their order: {string.Join(", ", order.Select(id => presets.FindById(id).Name))}");
        }
        if (!db.CollectionExists("Game")) return;
        var games = db.GetCollection<Game>();
        games.EnsureIndex(a => a.Id, true);
        var before = games.Count();
        var all = games.FindAll().ToList();
        // Each found again by its id: the index is in order.
        var found = all.Count(g => games.FindById(g.Id) != null);
        var id = Guid.Parse("00000000-1111-2222-3333-444444444444");
        games.Insert(new Game("Touched") { Id = id, PluginId = Steam, GameId = "1" });
        var touched = games.FindById(id);
        touched.Playtime = 42;
        games.Update(touched);
        var first = all.First();
        games.Delete(first.Id);
        Console.WriteLine($"{before} games, {found} found by id; then {games.Count()}, Touched played {games.FindById(id).Playtime} s, {first.Name} gone: {games.FindById(first.Id) == null}");
    }
}

// Playnite's own (Playnite.Database.FilterPresetsSettings): not in the SDK. Its collection is named after the class.
public class FilterPresetsSettings
{
    [BsonId(false)]
    public int Id { get; set; } = 0;
    public List<Guid> SortingOrder { get; set; } = new List<Guid>();
}
