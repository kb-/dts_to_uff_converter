function Info = writeuff58DX(fileName, UffDataSets, action) 
%WRITEUFF Writes UFF (Universal File Format) files of eight types: 
%   151, 15, 55, 58, 82, 1858, 164, 2420, and also the hybrid one, 58b 
%   Info = writeuff(fileName, UffDataSets, action) 
% 
%   - fileName:     name of the uff file to write data to (add or replace - 
%   see action parameter) 
%   - UffDataSets:  an array of structures; each structure holds one data 
%   set 
%                   (the data set between -1 and -1; Each structure, 
%                   UffDataSets{i}, has the field 
%                       .dsType 
%                       .binary 
%                   and some additional, data-set dependent fields (some of 
%                   them are optional) and are as follows: 
%                   %%1858 - for additional information not contained in  
%                           UFF58.  Should be placed just before uff58 if  
%                           used.  Items are defaulted except the following 
%                           are added: 
%                       .windowType             .AmpUnits 
%                       .NorMethod              .ordNumDataTypeQual 
%                       .ordDenomDataTypeQual   .zDataTypeQual 
%                       .samplingType           .zRPM 
%                       .zTime                  .zOrder 
%                       .NumSamples             .expWindowDamping 
%                   %%58 - for measurement data - function at dof (58). This 
%                   data is always saved as double precision data: 
%                       .x (time or frequency)  .measData               .d1 (descrip. 1) 
%                       .d2 (descrip. 2)        .date                   .functionType (see notes)  
%                       .rspNode                .rspDir                 .refNode        
%                       .refDir                  
%                       (Optional fields): 
%                       .ID_4                   .ID_5                   .loadCaseId   
%                       .rspEntName             .refEntName             .abscDataChar           .ordDataChar   
%                       .ordDenomDataChar       .abscUnitsLabel 
%                       .ordinateNumUnitsLabel  .ordinateDenumUnitsLabel 
%                       .zUnitsLabel            .zAxisValue 
%                       .abscLengthUnitsExponent.abscForceUnitsExponent 
%                       .abscTempUnitsExponent 
%                       .abscAxisLabel          .ordinateLengthUnitsExponent 
%                       .ordinateForceUnitsExponent                      .ordinateTempUnitsExponent 
%                       .ordinateAxisLabel 
% 
% 
%   - action:       (optional) 'add' (default) or 'replace' 
% 
%   - Info:         (optional) structure with the following fields: 
%                   .errcode    -   an array of error codes for each data 
%                                   ment to be written; 0 = no error otherwise an error occured in data 
%                                   set being written - see errmsg 
%                   .errmsg     -   error messages (cell array of strings) for each 
%                                   data set - empty if no error occured at specific data set 
%                   .nErrors    -   number of errors found (unsupported 
%                                   datasets, error writing data set,...) 
%                   .errorMsgs  -   error messages (empty if no error is found) 
% 
%   NOTES: r1..r6 are response vectors with node numbers in ROWS and 
%   direction in COLUMN (r1=x, r2=y,...,r6=rz). 
% 
%   functionType can be one of the following: 
%               0 - General or Unknown 
%               1 - Time Response 
%               2 - Auto Spectrum 
%               3 - Cross Spectrum 
%               4 - Frequency Response Function 
%               5 - Transmissibility 
%               6 - Coherence 
%               7 - Auto Correlation 
%               8 - Cross Correlation 
%               9 - Power Spectral Density (PSD) 
%               10 - Energy Spectral Density (ESD) 
%               11 - Probability Density Function 
%               12 - Spectrum 
%               13 - Cumulative Frequency Distribution 
%               14 - Peaks Valley 
%               15 - Stress/Cycles 
%               16 - Strain/Cycles 
%               17 - Orbit 
%               18 - Mode Indicator Function 
%               19 - Force Pattern 
%               20 - Partial Power 
%               21 - Partial Coherence 
%               22 - Eigenvalue 
%               23 - Eigenvector 
%               24 - Shock Response Spectrum 
%               25 - Finite Impulse Response Filter 
%               26 - Multiple Coherence 
%               27 - Order Function 
% 
%   analysisType can be one of the following: 
%               0: Unknown 
%               1: Static 
%               2: (supported) Normal Mode 
%               3: (supported) Complex eigenvalue first order 
%               4: Transient 
%               5: (supported) Frequency Response 
%               6: Buckling 
%               7: (supported) Complex eigenvalue second order 
% 
%   dataCharacter can be one of the following: 
%               0: Unknown 
%               1: Scalar 
%               2: 3 DOF Global Translation Vector 
%               3: 6 DOF Global Translation & Rotation Vector 
%               4: Symmetric Global Tensor 
% 
%   unitsCode can be one of the following: 
%               1 - SI: Meter (newton) 
%               2 - BG: Foot (pound f) 
%               3 - MG: Meter (kilogram f) 
%               4 - BA: Foot (poundal) 
%               5 - MM: mm (milli newton) 
%               6 - CM: cm (centi newton) 
%               7 - IN: Inch (pound f) 
%               8 - GM: mm (kilogram f) 
% 
%   functionType as well as other parameters are described in 
%   Test_Universal_File_Formats.pdf 
% 
%  
%
 
%-------------- 
% Check input arguments 
%-------------- 
error(nargchk(2,3,nargin)); 
if nargin < 3 || isempty(action) 
    action = 'add'; 
end 
if ~iscell(UffDataSets) 
    error('UffDataSets must be given as a cell array of structures'); 
end 
 
%-------------- 
% Open the file for writing 
%-------------- 
if strcmpi(action,'replace') 
    [fid,ermesage] = fopen(fileName,'w'); 
else 
    [fid,ermesage] = fopen(fileName,'a'); 
end 
if fid == -1, 
    error(['could not open file: ' fileName]); 
end 
 
%-------------- 
% Go through all the data sets and write each data set according to its type 
%-------------- 
nDataSets = length(UffDataSets); 
 
Info.errcode = zeros(nDataSets,1); 
Info.errmsg = cell(nDataSets,1); 
Info.nErrors = 0; 
 
for ii=1:nDataSets 
     %try 
        %% % 
        %% switch UffDataSets{ii}.dsType 
            %% case {15,82,55,1858,58,151,164,2420} 
    %fprintf(fid,'%6i%74s\n',-1,' '); 
    fprintf(fid,'%6i%74s \r\n',-1,' '); 
                %% switch UffDataSets{ii}.dsType 
                    %% 
                    %% case 1858 
                        %% Info.errmsg{ii} = write1858(fid,UffDataSets{ii}); 
                    %% case 58 
    %% Info.errmsg{ii} = write58(fid,UffDataSets{ii}); 
    
    % %%58 - Write data-set type 58 data 
if ispc 
    F_13 = '%13.4e'; 
    F_20 = '%20.11e'; 
else 
    F_13 = '%13.5e'; 
    F_20 = '%20.12e'; 
end 
errMessage = []; 

    %% if isempty(find(UffDataSets.functionType == [1 2 3 4 6 12])) 
        %% errMessage = ['Unsupported function type: ' num2str(UFF.functionType)]; 
        %% return 
    %% end 
    if ~isfield(UffDataSets{ii},'ID_4');  UffDataSets{ii}.ID_4 = 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'ID_5');  UffDataSets{ii}.ID_5 = 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'loadCaseId');  UffDataSets{ii}.loadCaseId = 0; end; 
    if ~isfield(UffDataSets{ii},'abscLengthUnitsExponent');  UffDataSets{ii}.abscLengthUnitsExponent = 0; end; 
    if ~isfield(UffDataSets{ii},'abscForceUnitsExponent');  UffDataSets{ii}.abscForceUnitsExponent = 0; end; 
    if ~isfield(UffDataSets{ii},'abscTempUnitsExponent');  UffDataSets{ii}.abscTempUnitsExponent = 0; end; 
    if ~isfield(UffDataSets{ii},'abscAxisLabel');  UffDataSets{ii}.abscAxisLabel = 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'ordinateLengthUnitsExponent');  UffDataSets{ii}.ordinateLengthUnitsExponent = 0; end; 
    if ~isfield(UffDataSets{ii},'ordinateForceUnitsExponent');  UffDataSets{ii}.ordinateForceUnitsExponent = 0; end; 
    if ~isfield(UffDataSets{ii},'ordinateTempUnitsExponent');  UffDataSets{ii}.ordinateTempUnitsExponent = 0; end; 
    if ~isfield(UffDataSets{ii},'ordinateAxisLabel');  UffDataSets{ii}.ordinateAxisLabel = 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'rspEntName');  UffDataSets{ii}.rspEntName = 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'refEntName');  UffDataSets{ii}.refEntName = 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'abscUnitsLabel');  UffDataSets{ii}.abscUnitsLabel= 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'ordinateNumUnitsLabel');  UffDataSets{ii}.ordinateNumUnitsLabel= 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'ordLenExp');  UffDataSets{ii}.ordLenExp = 0; end; 
    if ~isfield(UffDataSets{ii},'ordinateDenumUnitsLabel');  UffDataSets{ii}.ordinateDenumUnitsLabel= 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'ordDenomLenExp');  UffDataSets{ii}.ordDenomLenExp = 0; end; 
    if ~isfield(UffDataSets{ii},'zUnitsLabel');  UffDataSets{ii}.zUnitsLabel= 'NONE'; end; 
    if ~isfield(UffDataSets{ii},'zAxisValue');  UffDataSets{ii}.zAxisValue= 0; end;
    
    
   if UffDataSets{ii}.functionType == 1    % time response
        if ~isfield(UffDataSets{ii},'xAxisLabel');  UffDataSets{ii}.xAxisLabel='Durée (s)'; end;
        if ~isfield(UffDataSets{ii},'yAxisLabel');  UffDataSets{ii}.yAxisLabel='Accélération (g)'; end;
   elseif UffDataSets{ii}.functionType == 12 %functionType==12 spectrum
        if ~isfield(UffDataSets{ii},'xAxisLabel');  UffDataSets{ii}.xAxisLabel='Fréquence (Hz)'; end;
        if ~isfield(UffDataSets{ii},'yAxisLabel');  UffDataSets{ii}.yAxisLabel='Accélération (g eff)'; end;
   else
        if ~isfield(UffDataSets{ii},'xAxisLabel');  UffDataSets{ii}.xAxisLabel='NONE'; end;
        if ~isfield(UffDataSets{ii},'yAxisLabel');  UffDataSets{ii}.yAxisLabel='NONE'; end;
            
   end
     
   if UffDataSets{ii}.functionType == 1    % time response 
        if ~isfield(UffDataSets{ii},'abscDataChar');  UffDataSets{ii}.abscDataChar = 17; end; 
        if ~isfield(UffDataSets{ii},'ordDataChar');  UffDataSets{ii}.ordDataChar = 12; end; 
        if ~isfield(UffDataSets{ii},'ordDenomDataChar');  UffDataSets{ii}.ordDenomDataChar = 0; end; 
   else 
        if ~isfield(UffDataSets{ii},'abscDataChar');  UffDataSets{ii}.abscDataChar = 18; end; 
        if ~isfield(UffDataSets{ii},'ordDataChar');  UffDataSets{ii}.ordDataChar = 12; end; 
        if ~isfield(UffDataSets{ii},'ordDenomDataChar');  UffDataSets{ii}.ordDenomDataChar = 0; end; 
    end 
    % 
 %% isXEven = isempty(find((unique(UffDataSets{ii}.x(2:end)-UffDataSets{ii}.x(1:end-1))) < eps)); 
isXEven=1;    % 
    
        fprintf(fid,'%6i%74s \r\n',58,' '); 
    %end 
    if length(UffDataSets{ii}.d1)<=80, 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.d1);   %  line 1 
    else 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.d1(1:80));   %  line 1 
    end 
    if length(UffDataSets{ii}.d2)<=80, 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.d2);   %  line 2 
    else 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.d2(1:80));   %  line 2 
    end 
    if length(UffDataSets{ii}.date)<=80, 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.date);   %  line 3 
    else 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.date(1:80));   %  line 3 
    end 
    if length(UffDataSets{ii}.ID_4)<=80, 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.ID_4);   %  line 4 
    else 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.ID_4(1:80));   %  line 4 
    end 
    if length(UffDataSets{ii}.ID_5)<=80, 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.ID_5);   %  line 5 
    else 
        fprintf(fid,'%-80s \r\n',UffDataSets{ii}.ID_5(1:80));   %  line 5 
    end 
    
    %line 6 
    fprintf(fid,'%5i%10i%5i%10i %-10s%10i%4i %-10s%10i%4i \r\n',UffDataSets{ii}.functionType,0,0,UffDataSets{ii}.loadCaseId,UffDataSets{ii}.rspEntName,... 
        UffDataSets{ii}.rspNode,UffDataSets{ii}.rspDir,UffDataSets{ii}.refEntName,UffDataSets{ii}.refNode,UffDataSets{ii}.refDir);    % line 6 
    
    %% numpt = length(UffDataSets{ii}.measData);
    numpt = length(UffDataSets{ii}.measData); 
    
    % line 7 
    dx = UffDataSets{ii}.x(2) - UffDataSets{ii}.x(1); 
    isXEven=1;
    if imag(UffDataSets{ii}.measData)==0 
        % Always save as double precision 
        fprintf(fid,['%10i%10i%10i' F_13 F_13 F_13 '            \r\n'],2,numpt,isXEven,isXEven*UffDataSets{ii}.x(1),isXEven*dx,UffDataSets{ii}.zAxisValue); 
    else 
        % Always save as double precision 
        fprintf(fid,['%10i%10i%10i' F_13 F_13 F_13 '            \r\n'],6,numpt,isXEven,isXEven*UffDataSets{ii}.x(1),isXEven*dx,UffDataSets{ii}.zAxisValue); 
    end 
    
    
    % line 8 
    fprintf(fid,'%10i%5i%5i%5i %-20s %-20s              \r\n',UffDataSets{ii}.abscDataChar,0,0,0,UffDataSets{ii}.xAxisLabel,UffDataSets{ii}.abscUnitsLabel); 
    %  line 9 
    fprintf(fid,'%10i%5i%5i%5i %-20s %-20s              \r\n',UffDataSets{ii}.ordDataChar,UffDataSets{ii}.ordLenExp,0,0,UffDataSets{ii}.yAxisLabel,UffDataSets{ii}.ordinateNumUnitsLabel); 
    %                                                      ^--acceleration data 
    % line 10 
    % others: 0=unknown,8=displacement,11=velocity,13=excitation force,15=pressure 
    fprintf(fid,'%10i%5i%5i%5i %-20s %-20s              \r\n',UffDataSets{ii}.ordDenomDataChar,UffDataSets{ii}.ordDenomLenExp,0,0,'NONE',UffDataSets{ii}.ordinateDenumUnitsLabel); 
    %                                                      ^--excitation force data 
    % line 11 
    % others: 0=unknown,8=displacement,11=velocity,12=acceleration,15=pressure 
    fprintf(fid,'%10i%5i%5i%5i %-20s %-20s              \r\n',0,0,0,0,'NONE',UffDataSets{ii}.zUnitsLabel); 
    % 
    % line 12: % always as double precision 
    nOrdValues = length(UffDataSets{ii}.measData); 
    if imag(UffDataSets{ii}.measData)==0 
        if isXEven 
            newdata = UffDataSets{ii}.measData; 
        else 
            newdata = zeros(2*nOrdValues,1); 
            newdata(1:2:end-1) = UffDataSets{ii}.x; 
            newdata(2:2:end) = UffDataSets{ii}.measData; 
        end 
    else 
        if isXEven 
            newdata = zeros(2*nOrdValues,1); 
            newdata(1:2:end-1) = real(UffDataSets{ii}.measData); 
            newdata(2:2:end)   = imag(UffDataSets{ii}.measData); 
        else 
            newdata = zeros(3*nOrdValues,1); 
            newdata(1:3:end-2) = UffDataSets{ii}.x; 
            newdata(2:3:end-1) = real(UffDataSets{ii}.measData); 
            newdata(3:3:end)   = imag(UffDataSets{ii}.measData); 
        end 
    end 
    if UffDataSets{ii}.binary 
        if imag(UffDataSets{ii}.measData)==0 
            fwrite(fid,newdata, 'double'); 
        else 
            fwrite(fid,newdata, 'double'); 
        end 
    else    % ascii 
        if imag(UffDataSets{ii}.measData)==0    % real data 
            if isXEven 
                %% fprintf(fid,[F_20 F_20 F_20 F_20 F_20 F_20 ' \r\n'],newdata); 
                fprintf(fid,[F_13 F_13 F_13 F_13 F_13 F_13 ' \r\n'],newdata); 
            else 
                fprintf(fid,[F_13 F_20 F_13 F_20 ' \r\n'],newdata); 
            end 
            if rem(length(newdata),6)~=0, 
                fprintf(fid,' \r\n'); 
            end 
        else                        % complex data 
            if isXEven 
                fprintf(fid,[F_20 F_20 F_20 F_20 ' \r\n'],newdata); 
                if rem(length(newdata),4)~=0, 
                    fprintf(fid,' \r\n'); 
                end 
            else 
                fprintf(fid,[F_13 F_20 F_20 '\n'],newdata); 
                if rem(length(newdata),3)~=0, 
                    fprintf(fid,'\n'); 
                end 
            end 
        end 
end 

    
    
    
    
    
    
    
    
                %% end 
    fprintf(fid,'\r\n',' ')
    fprintf(fid,'%6i%74s \r\n',-1,' '); 
            %% otherwise 
                %% Info.errmsg{ii} = ['Unsupported data set: ' num2str(UffDataSets{ii}DataSets{ii}.dsType)]; 
%% end 
        % 
    %% catch 
        %% fclose(fid); 
        %% error(['Error writing UffDataSets{ii} file: ' fileName ': ' lasterr]); 
    %% end 
end 
fclose(fid); 

for ii=1:nDataSets 
    if ~isempty(Info.errmsg{ii}) 
        Info.errcode(ii) = 1; 
    end 
end 
Info.nErrors = length(find(Info.errcode)); 
Info.errorMsgs = Info.errmsg(find(Info.errcode)); 
 
 
 
 
 



