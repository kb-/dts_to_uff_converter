classdef DTS < handle
	properties
		STRING_TRUE = 'True';
		CH_INFO_CHANNEL_LOC = 1;
		CH_INFO_PROPORTIONAL = 2;
		CH_INFO_INVERTED = 3;
		CH_INFO_MEASURED_EXCITATION = 4;
		CH_INFO_FACTORY_EXCITATION = 5;
		CH_INFO_INITIAL_EU = 6;
		CH_INFO_ZERO_METHOD = 7;
		CH_INFO_ZERO_AVG_BEGIN = 8;
		CH_INFO_ZERO_AVG_END = 9;
		CH_INFO_TIME_FIRST_SAMPLE = 10;
		CH_INFO_START_REC_SAMPLE = 11;
		CH_INFO_ABSOLUTE_DISPLAY_ORDER = 12;
		CH_INFO_MAX = 12;

		CH_INFOMETA_SERIALNUMBER = 1;
		CH_INFOMETA_DESCRIPTION = 2;
		CH_INFOMETA_EU = 3;
		CH_INFOMETA_DISPLAYORDER = 4;
		CH_INFOMETA_MAX = 5;


		ZERO_METHOD_DIAG = 0;
		ZERO_METHOD_AVG = 1;
		ZERO_METHOD_NONE = 2;
		
		channelstart = [];
		channelInfo = [];
		triggerSampleNumber = [];
		sampleRate;
		preTestZeroLevelADC = [];
		scaleFactormV = [];
		scaleFactorEU = [];
		excitation = [];
		dataZeroLevelADC = [];
		channelInfoMetadata = {};
		fid = [];
		min_npts;
		fileCount;
		pn;
		
	end
	
	methods
		function o = DTS(varargin)
			%%initialize with DTS files directory
			%browse to folder if no argument
			if numel(varargin)==1
				o.pn = varargin{1};
			else
				o.pn = uigetdir('C:\DTS\SLICEWare', 'Select SLICEWare test folder');
			end
		end
		
		
		function [data] = readIdx(o,opt)
			%%read data from index array (opened file)
			% Input
			% opt structure 
			%   opt.idx = sample index array
			%	opt.tracks = [track numbers in ascending order,...](optional)
			if exist('opt')&&isfield(opt,'tracks')
                ntracks = numel(opt.tracks);
            else
				ntracks = o.fileCount;
			end
			data = zeros(numel(opt.idx),ntracks);
			for i=1:numel(opt.idx)
				opt.start = opt.idx(i);
				opt.stop = opt.idx(i);
				data(i,:) = o.read(opt);
			end
		end
		
		function [data, sampleRate, pn, channelInfoMetadata, timeOfFirstSamples, ADC] = read(o,opt)
			%%read data from opened files
			% Input
			% opt structure (optional)
			%   opt.start = start sample
			%   opt.stop = stop sample
			%	opt.tracks = [track numbers in ascending order,...]
			
			%BUG: opt.tracks: wrong data if not 1 or not successive numbers; ok with read_dts_folder
			
			if exist('opt')&&isfield(opt,'start')&&isfield(opt,'stop')
				part_file = true;
				skip = opt.start-1;
				blocksize = opt.stop - opt.start + 1;
			else
				skip = 0;
				part_file = false;
			end
			
			%% Get Min numper of points
			if part_file&&blocksize<o.min_npts
				min_npts = blocksize;
			else
				min_npts = o.min_npts;
			end
			
			nbchan = 0;
			ichan = 1;
			for k = 1:o.fileCount	
			%% Only get desired channels
				if exist('opt')&&isfield(opt,'tracks')&&isempty(find(opt.tracks==ichan))
					%skip
				else
					nbchan = nbchan+1;
				end
				ichan = ichan + 1;
			end				
			
			totalChannels = nbchan;
			ichan = 1;

			nbchan = 0;

			data = zeros(min_npts,totalChannels);
			if nargout>=6
				ADC = zeros(min_npts,totalChannels);
			end
			timeOfFirstSamples = zeros(1,totalChannels);
			
			for k = 1:o.fileCount
				%% Only get desired channels
				if exist('opt')&&isfield(opt,'tracks')&&isempty(find(opt.tracks==ichan))
					%skip
				else
					nbchan = nbchan+1;
					
					%% Get ADC Data	
					fseek(o.fid(k),o.channelstart(k),'bof');

					if ~part_file
						tempADC = fread(o.fid(k),'int16');
					else
						status = fseek(o.fid(k),skip*2,'cof');
						tempADC = fread(o.fid(k),blocksize,'int16');
					end

					%% prepare offset calculation
					timeOfFirstSample = (o.channelInfo(k, o.CH_INFO_START_REC_SAMPLE)- o.triggerSampleNumber(k) + skip)/o.sampleRate;
					timeOfFirstSamples(k) = timeOfFirstSample;
					if o.channelInfo(k,o.CH_INFO_ZERO_METHOD) == o.ZERO_METHOD_DIAG
						% Diagnostics Zero
						offset = (-o.preTestZeroLevelADC(k) * o.scaleFactormV(k)/o.scaleFactorEU(k)/o.excitation(k)) + o.channelInfo(k,o.CH_INFO_INITIAL_EU);    
					elseif o.channelInfo(k,o.CH_INFO_ZERO_METHOD) == o.ZERO_METHOD_AVG
						% Average Over Time
						offset = (-o.dataZeroLevelADC(k))*o.scaleFactormV(k)/o.scaleFactorEU(k)/o.excitation(k) + o.channelInfo(k,o.CH_INFO_INITIAL_EU);
					else
						% None
						offset = o.channelInfo(k,o.CH_INFO_INITIAL_EU);
					end

					%% Scale and offset ADC Data to get EU data
					temp = ((tempADC)*o.scaleFactormV(k)/o.scaleFactorEU(k)/o.excitation(k))+offset;		
					data(:,nbchan) = temp(1:min_npts) ;
					if nargout>=6
						ADC(:,nbchan) = tempADC(1:min_npts) ;
					end
				end
				ichan = ichan + 1;
			end
			sampleRate = o.sampleRate;
			pn = o.pn;
			channelInfoMetadata = o.channelInfoMetadata;
		end
		
		function o = close(o)
			%%Close files and reset info
			for k = 1:o.fileCount
				fclose(o.fid(k));
			end
			o.channelstart = [];
			o.channelInfo = [];
			o.triggerSampleNumber = [];
			o.sampleRate = [];
			o.preTestZeroLevelADC = [];
			o.scaleFactormV = [];
			o.scaleFactorEU = [];
			o.excitation = [];
			o.dataZeroLevelADC = [];
			o.channelInfoMetadata = {};
			o.fid = [];
			o.min_npts = [];
			o.fileCount = [];
		end
		
		function o = open(o)
		%%open DTS files and get info
		
		%% Locate Path

		currentFolder = pwd;
		cd(o.pn);

		%--------------------------------------------------------------------------
		%% Get Information From DTS File

		dts_fn = dir('*.dts'); dts_fn = dts_fn(1).name;

		totalChannels = 0;

		warning off MATLAB:iofun:UnsupportedEncoding;
		fid = fopen(dts_fn, 'r', 'n', 'Unicode');
		str = fread(fid, '*char')';
		fclose(fid);
		XMLHeaderLocs = strfind(str,'<?xml');
		if(length(XMLHeaderLocs) > 1)
			str = str(1:XMLHeaderLocs(2)-1);
			dts_fn = 'temp.dts';
			fid = fopen(dts_fn, 'w');
			encoded_str = unicode2native(str, 'UTF-16');
			fwrite(fid, encoded_str, 'uint8');
			fclose(fid);
		end


		xDoc = xmlread(fullfile(dts_fn));
		allModules = xDoc.getElementsByTagName('Module');
		o.channelInfo = zeros(xDoc.getElementsByTagName('AnalogInputChanel').getLength,o.CH_INFO_MAX);
		o.channelInfoMetadata = cell(o.CH_INFOMETA_MAX,xDoc.getElementsByTagName('AnalogInputChanel').getLength);
		o.excitation = zeros(xDoc.getElementsByTagName('AnalogInputChanel').getLength, 1);
		for kk = 0:allModules.getLength-1
			thisModule = allModules.item(kk);
			moduleAnalogInputChannels = thisModule.getElementsByTagName('AnalogInputChanel');
			for k = 0:moduleAnalogInputChannels.getLength-1
			   thisAnalogInputChannel = moduleAnalogInputChannels.item(k);       
			   channelInfoIndex = k+1+totalChannels;

			   %% get channel order   
			   o.channelInfo(channelInfoIndex, o.CH_INFO_CHANNEL_LOC) = str2double(thisAnalogInputChannel.getAttribute('AbsoluteDisplayOrder'));
			   %% get proportional
			   thisAttriubute = char(thisAnalogInputChannel.getAttribute('ProportionalToExcitation'));

			   if strcmp(thisAttriubute, o.STRING_TRUE)
				   thisSetting = 1;
			   else
				   thisSetting = 0;
			   end
			   o.channelInfo(channelInfoIndex, o.CH_INFO_PROPORTIONAL) = thisSetting;
			   %% get inverted
			   thisAttriubute = char(thisAnalogInputChannel.getAttribute('IsInverted'));

			   if strcmp(thisAttriubute, o.STRING_TRUE)
				   thisSetting = 1;
			   else
				   thisSetting = 0;
			   end
			   o.channelInfo(channelInfoIndex, o.CH_INFO_INVERTED) = thisSetting;
			   %% get excitation
			   o.channelInfo(channelInfoIndex, o.CH_INFO_MEASURED_EXCITATION) = str2double(thisAnalogInputChannel.getAttribute('MeasuredExcitationVoltage'));

			   o.channelInfo(channelInfoIndex, o.CH_INFO_FACTORY_EXCITATION) = str2double(thisAnalogInputChannel.getAttribute('FactoryExcitationVoltage'));

			   %% get initial eu
			   o.channelInfo(channelInfoIndex, o.CH_INFO_INITIAL_EU) = str2double(thisAnalogInputChannel.getAttribute('InitialEu'));

			   %% get zero method
			   thisAttriubute = char(thisAnalogInputChannel.getAttribute('ZeroMethod'));

			   if strcmp(thisAttriubute, 'UsePreCalZero')
				   thisSetting = o.ZERO_METHOD_DIAG;
			   elseif strcmp(thisAttriubute, 'AverageOverTime')
				   thisSetting = o.ZERO_METHOD_AVG;
			   else
				   thisSetting = o.ZERO_METHOD_NONE;
			   end
			   o.channelInfo(channelInfoIndex, o.CH_INFO_ZERO_METHOD) = thisSetting;

			   %% get average over time information
			   o.channelInfo(channelInfoIndex, o.CH_INFO_ZERO_AVG_BEGIN) = str2double(thisAnalogInputChannel.getAttribute('ZeroAverageWindowBegin'));
			   o.channelInfo(channelInfoIndex, o.CH_INFO_ZERO_AVG_END) = str2double(thisAnalogInputChannel.getAttribute('ZeroAverageWindowEnd'));
			   o.channelInfo(channelInfoIndex, o.CH_INFO_TIME_FIRST_SAMPLE) = str2double(thisAnalogInputChannel.getAttribute('TimeOfFirstSample'));
			   o.channelInfo(channelInfoIndex, o.CH_INFO_START_REC_SAMPLE) = str2double(thisModule.getAttribute('StartRecordSampleNumber'));
			   %% get absolute display order
			   o.channelInfo(channelInfoIndex, o.CH_INFO_ABSOLUTE_DISPLAY_ORDER) = str2double(thisAnalogInputChannel.getAttribute('AbsoluteDisplayOrder'));
			   %% get channel meta data
			   o.channelInfoMetadata(o.CH_INFOMETA_SERIALNUMBER, channelInfoIndex) = thisAnalogInputChannel.getAttribute('SerialNumber');
			   o.channelInfoMetadata(o.CH_INFOMETA_DESCRIPTION, channelInfoIndex) = thisAnalogInputChannel.getAttribute('Description');
			   o.channelInfoMetadata(o.CH_INFOMETA_EU, channelInfoIndex) = thisAnalogInputChannel.getAttribute('Eu');
			   o.channelInfoMetadata(o.CH_INFOMETA_DISPLAYORDER, channelInfoIndex) = thisAnalogInputChannel.getAttribute('AbsoluteDisplayOrder');
			end
			totalChannels = totalChannels + moduleAnalogInputChannels.getLength;
		end

		%--------------------------------------------------------------------------
		%% Find all .CHN files
		filelist = dir('*.chn');

		%% sort .chn files OSU YUN KANG
		names = {filelist.name};
		maxlen = max(cellfun(@length, names));
		padname = @(s) sprintf(['%0' num2str(maxlen) 's'], s);
		namesPadded = cellfun(padname, names, 'UniformOutput', false);
		[~, sortOrder] = sort(namesPadded);
		filelist = filelist(sortOrder);
		%%


		o.fileCount = numel(filelist);
		ichan = 1;
		%--------------------------------------------------------------------------
		%% Open all .CHN files
		o.min_npts = 2^64;
		nbchan = 0;
		for k = 1:o.fileCount
			fn = filelist(k).name;
			variable = [];
			%--------------------------------------------------------------------------
			%% Open File
			fid= fopen([o.pn '\' fn],'r'); % PL add backslash between path and filename
			if fid==-1
				h=errordlg('Can not open *.chn file');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return
			end
			%--------------------------------------------------------------------------
			%% Get DTS Header Data
			fseek(fid,0,'bof');
			magickey = fread(fid,[1],'uint32') ;

			if magickey ~= hex2dec('2C36351F') % PL changed != to ~= and used hex2dec conversion
				h=errordlg('Not a DTS *.chn file or file corrupted');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return 
			end

			%% Only get desired channels
			if exist('opt')&&isfield(opt,'tracks')&&isempty(find(opt.tracks==ichan))
				%skip
			else
				nbchan = nbchan+1;
				
				%% Get Min numper of points
				fseek(fid,16,'bof');
				npts = fread(fid,[1],'uint64') ;
				if(o.min_npts > npts)
					o.min_npts = npts;
				end
			end
			ichan = ichan + 1;
			% Close File
			fclose(fid);
		end

		totalChannels = nbchan;
		ichan = 1;

		nbchan = 0;
		timeOfFirstSamples = zeros(1,totalChannels);
		%--------------------------------------------------------------------------	
		%% Open all .CHN files
		for k = 1:o.fileCount
			fn = filelist(k).name;
			variable = [];
			%% Open File
			o.fid(k)= fopen([o.pn '\' fn],'r'); % PL add backslash between path and filename
			if o.fid(k)==-1
				h=errordlg('Can not open *.chn file');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return
			end
			
			%% Get DTS Header Data
			fseek(o.fid(k),0,'bof');
			magickey = fread(o.fid(k),[1],'uint32') ;
			
			if magickey ~= hex2dec('2C36351F') % PL changed != to ~= and used hex2dec conversion
				h=errordlg('Not a DTS *.chn file or file corrupted');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return 
			end

			nbchan = nbchan+1;
			fseek(o.fid(k),4,'bof');
			headerVersion = fread(o.fid(k),[1],'uint32');

			fseek(o.fid(k),8,'bof');
			o.channelstart(k) = fread(o.fid(k),[1],'uint64') ;

			fseek(o.fid(k),16,'bof');
			npts = fread(o.fid(k),[1],'uint64') ;

			fseek(o.fid(k),24,'bof');
			bitLength = fread(o.fid(k),[1],'uint32') ;

			fseek(o.fid(k),28,'bof');
			signed = fread(o.fid(k),[1],'uint32') ;

			fseek(o.fid(k),32,'bof');
			o.sampleRate = fread(o.fid(k),[1],'double');

			fseek(o.fid(k),40,'bof');
			numberOfTriggers = fread(o.fid(k),[1],'uint16');
			N = numberOfTriggers*8;

			fseek(o.fid(k),42,'bof');
			o.triggerSampleNumber(k) = fread(o.fid(k),[1],'int64') ;

			fseek(o.fid(k),N+42,'bof');
			o.preTestZeroLevelADC(k) = fread(o.fid(k),[1],'int32') ;

			fseek(o.fid(k),N+46,'bof');
			removedADC = fread(o.fid(k),[1],'int32') ;

			fseek(o.fid(k),N+50,'bof');
			preTestDiagnosticsLevelADC = fread(o.fid(k),[1],'int32') ;

			fseek(o.fid(k),N+54,'bof');
			preTestNoise = fread(o.fid(k),[1],'double') ;

			fseek(o.fid(k),N+62,'bof');
			postTestZeroLevelADC = fread(o.fid(k),[1],'int32') ;

			fseek(o.fid(k),N+66,'bof');
			postTestDiagnosticsLevelADC = fread(o.fid(k),[1],'int32') ;

			fseek(o.fid(k),N+70,'bof');
			o.dataZeroLevelADC(k) = fread(o.fid(k),[1],'int32') ;

			fseek(o.fid(k),(N+74),'bof');
			o.scaleFactormV(k) = fread(o.fid(k),[1],'double');

			fseek(o.fid(k),(N+82),'bof');
			o.scaleFactorEU(k) = fread(o.fid(k),[1],'double');
			%% prepare inverted
			if o.channelInfo(k, o.CH_INFO_INVERTED) == 1
				%% inverted
				o.scaleFactormV(k) = -o.scaleFactormV(k);
			end
			%% prepare excitation calculation

			if o.channelInfo(k,o.CH_INFO_PROPORTIONAL) == 0
				o.excitation(k) = 1;
			else
				if isnan(o.channelInfo(k,o.CH_INFO_FACTORY_EXCITATION))
					o.excitation(k) = o.channelInfo(k,o.CH_INFO_MEASURED_EXCITATION);
				else
					o.excitation(k) = o.channelInfo(k,o.CH_INFO_FACTORY_EXCITATION);
				end
			end
			%% Close File
			%fclose(fid);
			ichan = ichan + 1;
			
			%% Get Min numper of points
			fseek(fid,16,'bof');
			npts = fread(fid,[1],'uint64') ;
			if(o.min_npts > npts)
				o.min_npts = npts;
			end
		end
		%% Cleanup
		cd(currentFolder);
		end
		
		%read data and close files
		function  [data, sampleRate, pn, channelInfoMetadata, timeOfFirstSamples, ADC] = read_dts_folder(o,opt)
		%--------------------------------------------------------------------------
		% read_diadem_chan
		%   This function will read all the channels in a DTS test folder.  The the
		%	header info from the .CHN files in the test folder along with the raw
		%	DTS data are read and scaled by the EU per count value and the offset
		%	stored in the .CHN header.
		%
		%   The original code was taken from the Matlab Central File Exchange. It
		%   was provided by Wolfgang Ritz, 3/2011. I have made some minor
		%   modifications to make it work for the DTS SLICEWare data folder,
		%   plus translated a few of the German comments.
		%
		% Input
		% opt structure (optional)
		%   opt.start = start sample
		%   opt.stop = stop sample
		%	opt.tracks = [track numbers in ascending order,...]
		% Input (file explorer)
		%	DTS Test Folder
		%   DTS channel files
		% Output
		%   data     	- array (npts,nchan) of EU data
		%   sampleRate	- sample rate
		%   pn       	- input path name
		%   channelInfoMetadata
		%
		% History
		%   CPB 07/25/14 - original code
		%	OE EDF 19/10/17 - partial file read, optimisation if ADC data not requested
		%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
		%--------------------------------------------------------------------------

		%% Locate Path

		currentFolder = pwd;
		cd(o.pn);

		%--------------------------------------------------------------------------
		%% Get Information From DTS File

		dts_fn = dir('*.dts'); dts_fn = dts_fn(1).name;

		totalChannels = 0;

		warning off MATLAB:iofun:UnsupportedEncoding;
		fid = fopen(dts_fn, 'r', 'n', 'Unicode');
		str = fread(fid, '*char')';
		fclose(fid);
		XMLHeaderLocs = strfind(str,'<?xml');
		if(length(XMLHeaderLocs) > 1)
			str = str(1:XMLHeaderLocs(2)-1);
			dts_fn = 'temp.dts';
			fid = fopen(dts_fn, 'w');
			encoded_str = unicode2native(str, 'UTF-16');
			fwrite(fid, encoded_str, 'uint8');
			fclose(fid);
		end


		xDoc = xmlread(fullfile(dts_fn));
		allModules = xDoc.getElementsByTagName('Module');
		channelInfo = zeros(xDoc.getElementsByTagName('AnalogInputChanel').getLength,o.CH_INFO_MAX);
		channelInfoMetadata = cell(o.CH_INFOMETA_MAX,xDoc.getElementsByTagName('AnalogInputChanel').getLength);
		excitation = zeros(xDoc.getElementsByTagName('AnalogInputChanel').getLength, 1);
		for kk = 0:allModules.getLength-1
			thisModule = allModules.item(kk);
			moduleAnalogInputChannels = thisModule.getElementsByTagName('AnalogInputChanel');
			for k = 0:moduleAnalogInputChannels.getLength-1
			   thisAnalogInputChannel = moduleAnalogInputChannels.item(k);       
			   channelInfoIndex = k+1+totalChannels;

			   %% get channel order   
			   channelInfo(channelInfoIndex, o.CH_INFO_CHANNEL_LOC) = str2double(thisAnalogInputChannel.getAttribute('AbsoluteDisplayOrder'));
			   %% get proportional
			   thisAttriubute = char(thisAnalogInputChannel.getAttribute('ProportionalToExcitation'));

			   if strcmp(thisAttriubute, o.STRING_TRUE)
				   thisSetting = 1;
			   else
				   thisSetting = 0;
			   end
			   channelInfo(channelInfoIndex, o.CH_INFO_PROPORTIONAL) = thisSetting;
			   %% get inverted
			   thisAttriubute = char(thisAnalogInputChannel.getAttribute('IsInverted'));

			   if strcmp(thisAttriubute, o.STRING_TRUE)
				   thisSetting = 1;
			   else
				   thisSetting = 0;
			   end
			   channelInfo(channelInfoIndex, o.CH_INFO_INVERTED) = thisSetting;
			   %% get excitation
			   channelInfo(channelInfoIndex, o.CH_INFO_MEASURED_EXCITATION) = str2double(thisAnalogInputChannel.getAttribute('MeasuredExcitationVoltage'));

			   channelInfo(channelInfoIndex, o.CH_INFO_FACTORY_EXCITATION) = str2double(thisAnalogInputChannel.getAttribute('FactoryExcitationVoltage'));

			   %% get initial eu
			   channelInfo(channelInfoIndex, o.CH_INFO_INITIAL_EU) = str2double(thisAnalogInputChannel.getAttribute('InitialEu'));

			   %% get zero method
			   thisAttriubute = char(thisAnalogInputChannel.getAttribute('ZeroMethod'));

			   if strcmp(thisAttriubute, 'UsePreCalZero')
				   thisSetting = o.ZERO_METHOD_DIAG;
			   elseif strcmp(thisAttriubute, 'AverageOverTime')
				   thisSetting = o.ZERO_METHOD_AVG;
			   else
				   thisSetting = o.ZERO_METHOD_NONE;
			   end
			   channelInfo(channelInfoIndex, o.CH_INFO_ZERO_METHOD) = thisSetting;

			   %% get average over time information
			   channelInfo(channelInfoIndex, o.CH_INFO_ZERO_AVG_BEGIN) = str2double(thisAnalogInputChannel.getAttribute('ZeroAverageWindowBegin'));
			   channelInfo(channelInfoIndex, o.CH_INFO_ZERO_AVG_END) = str2double(thisAnalogInputChannel.getAttribute('ZeroAverageWindowEnd'));
			   channelInfo(channelInfoIndex, o.CH_INFO_TIME_FIRST_SAMPLE) = str2double(thisAnalogInputChannel.getAttribute('TimeOfFirstSample'));
			   channelInfo(channelInfoIndex, o.CH_INFO_START_REC_SAMPLE) = str2double(thisModule.getAttribute('StartRecordSampleNumber'));
			   %% get absolute display order
			   channelInfo(channelInfoIndex, o.CH_INFO_ABSOLUTE_DISPLAY_ORDER) = str2double(thisAnalogInputChannel.getAttribute('AbsoluteDisplayOrder'));
			   %% get channel meta data
			   channelInfoMetadata(o.CH_INFOMETA_SERIALNUMBER, channelInfoIndex) = thisAnalogInputChannel.getAttribute('SerialNumber');
			   channelInfoMetadata(o.CH_INFOMETA_DESCRIPTION, channelInfoIndex) = thisAnalogInputChannel.getAttribute('Description');
			   channelInfoMetadata(o.CH_INFOMETA_EU, channelInfoIndex) = thisAnalogInputChannel.getAttribute('Eu');
			   channelInfoMetadata(o.CH_INFOMETA_DISPLAYORDER, channelInfoIndex) = thisAnalogInputChannel.getAttribute('AbsoluteDisplayOrder');
			end
			totalChannels = totalChannels + moduleAnalogInputChannels.getLength;
		end

		%--------------------------------------------------------------------------
		%% Find all .CHN files
		filelist = dir('*.chn');

		%% sort .chn files OSU YUN KANG
		names = {filelist.name};
		maxlen = max(cellfun(@length, names));
		padname = @(s) sprintf(['%0' num2str(maxlen) 's'], s);
		namesPadded = cellfun(padname, names, 'UniformOutput', false);
		[~, sortOrder] = sort(namesPadded);
		filelist = filelist(sortOrder);
		%%


		fileCount = numel(filelist);
		ichan = 1;

		if exist('opt')&&isfield(opt,'start')&&isfield(opt,'stop')
			part_file = true;
			skip = opt.start-1;
			blocksize = opt.stop - opt.start + 1;
		else
			skip = 0;
			part_file = false;
		end

		%--------------------------------------------------------------------------
		%% Open all .CHN files
		min_npts = 2^64;
		nbchan = 0;
		for k = 1:fileCount
			fn = filelist(k).name;
			variable = [];
			%--------------------------------------------------------------------------
			%% Open File
			fid= fopen([o.pn '\' fn],'r'); % PL add backslash between path and filename
			if fid==-1
				h=errordlg('Can not open *.chn file');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return
			end
			%--------------------------------------------------------------------------
			%% Get DTS Header Data
			fseek(fid,0,'bof');
			magickey = fread(fid,[1],'uint32') ;

			if magickey ~= hex2dec('2C36351F') % PL changed != to ~= and used hex2dec conversion
				h=errordlg('Not a DTS *.chn file or file corrupted');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return 
			end

			%% Only get desired channels
			if exist('opt')&&isfield(opt,'tracks')&&isempty(find(opt.tracks==ichan))
				%skip
			else
				nbchan = nbchan+1;
				
				%% Get Min numper of points
				fseek(fid,16,'bof');
				npts = fread(fid,[1],'uint64') ;
				if(min_npts > npts)
					if ~part_file
						min_npts = npts;
					else
						min_npts = blocksize;
					end
				end
			end
			ichan = ichan + 1;
			% Close File
			fclose(fid);
		end

		totalChannels = nbchan;
		ichan = 1;

		nbchan = 0;

		data = zeros(min_npts,totalChannels);
		if nargout>=6
			ADC = zeros(min_npts,totalChannels);
		end
		timeOfFirstSamples = zeros(1,totalChannels);
		%--------------------------------------------------------------------------	
		%% Open all .CHN files
		for k = 1:fileCount
			fn = filelist(k).name;
			variable = [];
			%% Open File
			fid= fopen([o.pn '\' fn],'r'); % PL add backslash between path and filename
			if fid==-1
				h=errordlg('Can not open *.chn file');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return
			end
			
			%% Get DTS Header Data
			fseek(fid,0,'bof');
			magickey = fread(fid,[1],'uint32') ;
			
			if magickey ~= hex2dec('2C36351F') % PL changed != to ~= and used hex2dec conversion
				h=errordlg('Not a DTS *.chn file or file corrupted');
				uiwait(h);
				variable='Function Aborted';
				cd(currentFolder);
				return 
			end
			
			%% Only get desired channels
			if exist('opt')&&isfield(opt,'tracks')&&isempty(find(opt.tracks==ichan))
				%skip
			else
				nbchan = nbchan+1;
				fseek(fid,4,'bof');
				headerVersion = fread(fid,[1],'uint32');

				fseek(fid,8,'bof');
				channelstart = fread(fid,[1],'uint64') ;

				fseek(fid,16,'bof');
				npts = fread(fid,[1],'uint64') ;

				fseek(fid,24,'bof');
				bitLength = fread(fid,[1],'uint32') ;

				fseek(fid,28,'bof');
				signed = fread(fid,[1],'uint32') ;

				fseek(fid,32,'bof');
				sampleRate = fread(fid,[1],'double');

				fseek(fid,40,'bof');
				numberOfTriggers = fread(fid,[1],'uint16');
				N = numberOfTriggers*8;

				fseek(fid,42,'bof');
				triggerSampleNumber = fread(fid,[1],'int64') ;

				fseek(fid,N+42,'bof');
				preTestZeroLevelADC = fread(fid,[1],'int32') ;

				fseek(fid,N+46,'bof');
				removedADC = fread(fid,[1],'int32') ;

				fseek(fid,N+50,'bof');
				preTestDiagnosticsLevelADC = fread(fid,[1],'int32') ;

				fseek(fid,N+54,'bof');
				preTestNoise = fread(fid,[1],'double') ;

				fseek(fid,N+62,'bof');
				postTestZeroLevelADC = fread(fid,[1],'int32') ;

				fseek(fid,N+66,'bof');
				postTestDiagnosticsLevelADC = fread(fid,[1],'int32') ;

				fseek(fid,N+70,'bof');
				dataZeroLevelADC = fread(fid,[1],'int32') ;

				fseek(fid,(N+74),'bof');
				scaleFactormV = fread(fid,[1],'double');

				fseek(fid,(N+82),'bof');
				scaleFactorEU = fread(fid,[1],'double');
				%% prepare inverted
				if channelInfo(k, o.CH_INFO_INVERTED) == 1
					%% inverted
					scaleFactormV = -scaleFactormV;
				end
				%% prepare excitation calculation

				if channelInfo(k,o.CH_INFO_PROPORTIONAL) == 0
					excitation(k) = 1;
				else
					if isnan(channelInfo(k,o.CH_INFO_FACTORY_EXCITATION))
						excitation(k) = channelInfo(k,o.CH_INFO_MEASURED_EXCITATION);
					else
						excitation(k) = channelInfo(k,o.CH_INFO_FACTORY_EXCITATION);
					end
				end
				%% Get ADC Data	
				fseek(fid,channelstart,'bof');

				if ~part_file
					tempADC = fread(fid,'int16');
				else
					status = fseek(fid,skip,'cof');
					tempADC = fread(fid,blocksize,'int16');
				end

				%% prepare offset calculation
				timeOfFirstSample = (channelInfo(k, o.CH_INFO_START_REC_SAMPLE)- triggerSampleNumber + skip)/sampleRate;
				timeOfFirstSamples(k) = timeOfFirstSample;
				if channelInfo(k,o.CH_INFO_ZERO_METHOD) == o.ZERO_METHOD_DIAG
					% Diagnostics Zero
					offset = (-preTestZeroLevelADC * scaleFactormV/scaleFactorEU/excitation(k)) + channelInfo(k,o.CH_INFO_INITIAL_EU);    
				elseif channelInfo(k,o.CH_INFO_ZERO_METHOD) == o.ZERO_METHOD_AVG
					% Average Over Time
					offset = (-dataZeroLevelADC)*scaleFactormV/scaleFactorEU/excitation(k) + channelInfo(k,o.CH_INFO_INITIAL_EU);
				else
					% None
					offset = channelInfo(k,o.CH_INFO_INITIAL_EU);
				end

				%% Scale and offset ADC Data to get EU data
				temp = ((tempADC)*scaleFactormV/scaleFactorEU/excitation(k))+offset;		
				name = ['CH' num2str(ichan)];
				data(:,nbchan) = temp(1:min_npts) ;
				if nargout>=6
					ADC(:,nbchan) = tempADC(1:min_npts) ;
				end
			end
			%% Close File
			fclose(fid);
			ichan = ichan + 1;
		end
		%% Cleanup
		cd(currentFolder);
		pause(1)
		end
	end
end