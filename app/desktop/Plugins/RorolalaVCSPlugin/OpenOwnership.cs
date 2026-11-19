using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>What the open check reads about a file's ownership.</summary>
/// <remarks>
/// The check is a read of the local copy the Workspace has fetched, so what answers it is
/// <see cref="OwnershipScope"/> in the program and a script in a test. Nothing else about an open is
/// asked here, which keeps the decision testable without a Vault.
/// </remarks>
internal interface IOwnershipLookup
{
    /// <summary>Who holds the file at a path, or nothing when the path is in no Workspace.</summary>
    /// <param name="path">The file, as the program names it.</param>
    /// <returns>What holds it, or nothing when there is nothing to read.</returns>
    Task<EntryLock?> LookAsync(string path);
}

/// <summary>What the open check needs from the command that takes ownership.</summary>
internal interface IOwnershipHold
{
    /// <summary>Takes the file at a path, or says why it could not be taken.</summary>
    /// <param name="path">The file, as the program names it.</param>
    /// <returns>Whether it was taken, and what to say when it was not.</returns>
    Task<OwnershipHoldOutcome> HoldAsync(string path);
}

/// <summary>What a hold attempt came to.</summary>
/// <param name="Done">Whether the file was taken.</param>
/// <param name="Said">What the command said when it was not taken.</param>
internal sealed record OwnershipHoldOutcome(bool Done, string Said);

/// <summary>
/// The check a file passes before the Desktop opens it: who holds it, and what to do about that.
/// </summary>
/// <remarks>
/// Opening a tracked file is where this plugin can still say something before another program takes
/// the file over. A file another account holds is opened with the warning that a change would be
/// recorded as an error; a file nobody holds is offered the hold that gives the reader the right to
/// edit it, or opened as it is; a hold that failed can be tried again, opened past, or left alone.
/// <para>
/// The ownership is read afresh for the click rather than from a listing's kept answer: the listing's
/// answer may not be there yet, and a question about this file is one this plugin can afford to ask.
/// The read is a local one, so nothing is reached for over the network.
/// </para>
/// <para>
/// Only a single file is reviewed, which is why the hook checks the request's target rather than a
/// batch: the browser asks only for a single open, and a batch is opened as it always was.
/// </para>
/// </remarks>
internal sealed class OwnershipOpenHook : IOpenHook
{
    /// <summary>What the file is called when it is held by somebody else.</summary>
    private const string HeldTitle = "rorolala_vcs.open.held_title";

    /// <summary>What the warning says about the file another account holds.</summary>
    private const string HeldMessage = "rorolala_vcs.open.held_message";

    /// <summary>What the file is called when nobody holds it.</summary>
    private const string FreeTitle = "rorolala_vcs.open.free_title";

    /// <summary>What the offer says about the file nobody holds.</summary>
    private const string FreeMessage = "rorolala_vcs.open.free_message";

    /// <summary>What the failure is called.</summary>
    private const string FailedTitle = "rorolala_vcs.open.failed_title";

    /// <summary>What the failure says.</summary>
    private const string FailedMessage = "rorolala_vcs.open.failed_message";

    /// <summary>The answer that opens the file without taking it.</summary>
    private const string OpenFile = "rorolala_vcs.open.open_file";

    /// <summary>The answer that takes the file and then opens it.</summary>
    private const string HoldAndOpen = "rorolala_vcs.open.hold_and_open";

    /// <summary>The answer that opens the file without holding it.</summary>
    private const string OpenWithoutHolding = "rorolala_vcs.open.open_without_holding";

    /// <summary>The answer that tries the hold again.</summary>
    private const string Retry = "rorolala_vcs.open.retry";

    /// <summary>What the host calls the answer that does nothing.</summary>
    private const string Cancel = "window.dialog.cancel";

    /// <summary>Where the ownership of a file is read.</summary>
    private readonly IOwnershipLookup _lookup;

    /// <summary>Which takes a file when the reader chooses to.</summary>
    private readonly IOwnershipHold _hold;

    /// <summary>Where the questions are put and the answers are read.</summary>
    private readonly IDialogs _dialogs;

    /// <summary>Where a hold that failed says why.</summary>
    private readonly ILog _log;

    /// <summary>Makes the check over what it reads, what it runs and where it asks.</summary>
    /// <param name="lookup">Where the ownership of a file is read.</param>
    /// <param name="hold">Which takes a file when the reader chooses to.</param>
    /// <param name="dialogs">Where the questions are put.</param>
    /// <param name="log">Where a hold that failed says why.</param>
    public OwnershipOpenHook(
        IOwnershipLookup lookup,
        IOwnershipHold hold,
        IDialogs dialogs,
        ILog log
    )
    {
        ArgumentNullException.ThrowIfNull(lookup);
        ArgumentNullException.ThrowIfNull(hold);
        ArgumentNullException.ThrowIfNull(dialogs);
        ArgumentNullException.ThrowIfNull(log);

        _lookup = lookup;
        _hold = hold;
        _dialogs = dialogs;
        _log = log;
    }

    /// <inheritdoc />
    public OpenStage Stage => OpenStage.CanOpen;

    /// <inheritdoc />
    public async Task<OpenRequest?> OnOpenAsync(OpenRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        if (request.Target.Kind is not EntryKind.File)
        {
            return request;
        }

        var path = request.Target.Path;
        var held = await Look(path).ConfigureAwait(false);

        return held?.Kind switch
        {
            EntryLockKind.Held => await Warn(request, held.Holder).ConfigureAwait(false),
            EntryLockKind.Free => await Offer(request).ConfigureAwait(false),
            _ => request,
        };
    }

    /// <summary>Warns that another account holds the file, and opens it only if the reader says so.</summary>
    /// <param name="request">The open being reviewed.</param>
    /// <param name="holder">The account that holds the file.</param>
    /// <returns>The request when the reader chose to open, or nothing.</returns>
    private async Task<OpenRequest?> Warn(OpenRequest request, string? holder)
    {
        var open = new DialogChoice(RolaI18N.Get(OpenFile), () => { });
        var cancel = new DialogChoice(RolaI18N.Get(Cancel), () => { });

        var answer = await _dialogs
            .Ask(
                new Choice(
                    RolaI18N.Get(HeldTitle),
                    RolaI18N.Get(HeldMessage, holder ?? string.Empty),
                    [open, cancel]
                )
            )
            .ConfigureAwait(false);

        return ReferenceEquals(answer, open) ? request : null;
    }

    /// <summary>Offers to hold a file nobody holds, or to open it as it is.</summary>
    /// <param name="request">The open being reviewed.</param>
    /// <returns>The request when the reader chose an open, or nothing.</returns>
    private async Task<OpenRequest?> Offer(OpenRequest request)
    {
        var hold = new DialogChoice(RolaI18N.Get(HoldAndOpen), () => { });
        var open = new DialogChoice(RolaI18N.Get(OpenWithoutHolding), () => { });
        var cancel = new DialogChoice(RolaI18N.Get(Cancel), () => { });

        var answer = await _dialogs
            .Ask(new Choice(RolaI18N.Get(FreeTitle), RolaI18N.Get(FreeMessage), [hold, open, cancel]))
            .ConfigureAwait(false);

        if (answer is null || ReferenceEquals(answer, cancel))
        {
            return null;
        }

        return ReferenceEquals(answer, open) ? request : await Acquire(request).ConfigureAwait(false);
    }

    /// <summary>Takes the file, then opens it, or says what the failure means.</summary>
    /// <param name="request">The open being reviewed.</param>
    /// <returns>The request when the file was taken, or the dialog that says what happened.</returns>
    private async Task<OpenRequest?> Acquire(OpenRequest request)
    {
        if (await TryHold(request.Target.Path).ConfigureAwait(false))
        {
            return request;
        }

        var held = await Look(request.Target.Path).ConfigureAwait(false);

        // What the file became matters: another account may have taken it between the question and
        // the run, which is a warning rather than a failed acquisition.
        return held?.Kind is EntryLockKind.Held
            ? await Warn(request, held.Holder).ConfigureAwait(false)
            : await Failed(request).ConfigureAwait(false);
    }

    /// <summary>Asks what to do about a hold that failed, and retries while the reader asks it to.</summary>
    /// <param name="request">The open being reviewed.</param>
    /// <returns>The request when the reader chose to open, or nothing.</returns>
    private async Task<OpenRequest?> Failed(OpenRequest request)
    {
        while (true)
        {
            var retry = new DialogChoice(RolaI18N.Get(Retry), () => { });
            var open = new DialogChoice(RolaI18N.Get(OpenWithoutHolding), () => { });
            var cancel = new DialogChoice(RolaI18N.Get(Cancel), () => { });

            var answer = await _dialogs
                .Ask(
                    new Choice(
                        RolaI18N.Get(FailedTitle),
                        RolaI18N.Get(FailedMessage),
                        [retry, open, cancel]
                    )
                )
                .ConfigureAwait(false);

            if (answer is null || ReferenceEquals(answer, cancel))
            {
                return null;
            }

            if (ReferenceEquals(answer, open))
            {
                return request;
            }

            if (await TryHold(request.Target.Path).ConfigureAwait(false))
            {
                return request;
            }

            var held = await Look(request.Target.Path).ConfigureAwait(false);

            if (held?.Kind is EntryLockKind.Held)
            {
                return await Warn(request, held.Holder).ConfigureAwait(false);
            }
        }
    }

    /// <summary>Takes one file, saying why when it could not be taken.</summary>
    /// <param name="path">The file to take.</param>
    /// <returns>Whether it was taken.</returns>
    private async Task<bool> TryHold(string path)
    {
        try
        {
            var outcome = await _hold.HoldAsync(path).ConfigureAwait(false);

            if (outcome.Done)
            {
                return true;
            }

            _log.Warn($"holding `{path}` failed: {outcome.Said}");

            return false;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"holding `{path}` failed: {error.Message}");

            return false;
        }
    }

    /// <summary>Reads the ownership of one file, answering nothing when it cannot be read.</summary>
    /// <param name="path">The file to ask about.</param>
    /// <returns>What holds it, or nothing when nothing can be read.</returns>
    private async Task<EntryLock?> Look(string path)
    {
        try
        {
            return await _lookup.LookAsync(path).ConfigureAwait(false);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            // A file whose ownership cannot be read is opened as before: the check is an offer of
            // protection, and a broken read must not become a file that cannot be opened.
            _log.Warn($"the ownership of `{path}` could not be read: {error.Message}");

            return null;
        }
    }
}
