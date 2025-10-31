% Low-memory export with same field content (except shorter .x)
d = DTS();
d.open();
[~, fs_probe, ~, channelInfoMetadata] = d.read(struct('start',1,'stop',1)); % probe only
ntracks = size(channelInfoMetadata,2);

[filename, pathname] = uigetfile('*.txt','Liste des voies');
if isequal(filename,0), return; end
fid = fopen(fullfile(pathname, filename),'r');
fdata_n = textscan(fid, '%q', 'delimiter',',');
fclose(fid);

[file, outdir, filteridx] = uiputfile({'*.uff';'*.uffDX'},'Sauvegarde du signal');
if isequal(file,0), return; end
outpath = fullfile(outdir, file);
hb = waitbar(0,'Running...','CreateCancelBtn','setappdata(gcbf,''canceling'',1)');
setappdata(hb,'canceling',0);

if filteridx==1  % ---- General UFF (same fields as your code) ----
    action = 'replace';  % first write creates/truncates
    for i = 1:ntracks
		if getappdata(hb,'canceling')
			delete(hb);
			break
		end
        [data, fs, ~, channelInfoMetadata_i] = d.read(struct('tracks', i)'); %, 'start',1,'stop', 10000
		if getappdata(hb,'canceling')
			delete(hb);
			break
		end
        s = struct;
        s.dx  = 1/fs;
        s.measData = data;                          % same type as read (your original)
        s.rspEntName = fdata_n{1}{i};
        s.d1 = '';
        s.d2 = ['Pt=' s.rspEntName ';'];
        s.date = '';
        s.functionType = 1;
        s.loadCaseId = 0;
        s.rspNode = 0;
        s.rspDir = 0;
        s.refEntName = 'NONE      ';               % keep padded value you had
        s.refNode = 1;
        s.refDir = 0;
        s.xmin = 0;
        s.abscAxisLabel = 'Time                ';
        s.abscUnitsLabel = 's                                ';
        s.ordinateNumUnitsLabel = channelInfoMetadata{3,i};
        s.ordinateAxisLabel = s.rspEntName;
        s.x = [0, 1/fs];                           % <<< short .x (was 0:1/fs:... before)
        s.dsType = 58;
        s.binary = 0;

        writeuff(outpath, {s}, action);
        action = 'add';
		waitbar(i/ntracks,hb,[s.rspEntName ' exported']);
        clear data s
    end

elseif filteridx==2  % ---- DYNAMX UFF (writeuff58DX) with same fields ----
    action = 'replace';
    for i = 1:ntracks
		if getappdata(hb,'canceling')
			delete(hb);
			break
		end
        [data, fs, ~, channelInfoMetadata_i] = d.read(struct('tracks', i)'); %, 'start',1,'stop', 10000
		if getappdata(hb,'canceling')
			delete(hb);
			break
		end

        s = struct;
        s.dx  = 1/fs;                               % keep field since you had it
        s.measData = data;
        s.rspEntName = fdata_n{1}{i};
        s.ID_5 = '';
        s.d1 = '';
        s.d2 = ['Pt=' s.rspEntName ';'];
        s.date = '';
        s.ID_4 = '';
        s.functionType = 1;
        s.loadCaseId = 0;
        s.rspNode = 0;
        s.rspDir = 0;
        s.refEntName = '';                          % same as your DX branch
        s.refNode = 0;
        s.refDir = 0;
        s.xmin = 0;
        s.x = [0, 1/fs];                            % <<< short .x (writer only uses x(1) & dx)
        s.dsType = 58;
        s.binary = 0;

        writeuff58DX(outpath, {s}, action);
        action = 'add';
        clear data s
    end
end
delete(hb);